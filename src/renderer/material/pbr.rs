//! PBR (Physically Based Rendering) material

use super::traits::{Material, ModelUniform};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::{PipelineBuilder, Vertex};
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::renderer::light::Light;
use crate::renderer::viewer::{CameraUniform, Viewer};
use glam::Mat4;

/// PBR material uniform data.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PbrUniform {
    /// Base color (albedo) RGBA.
    pub base_color: [f32; 4],
    /// Emissive color RGB + padding.
    pub emissive: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Ambient occlusion factor.
    pub ao: f32,
    /// Padding.
    pub _padding: f32,
}

impl Default for PbrUniform {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            emissive: [0.0, 0.0, 0.0, 0.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            _padding: 0.0,
        }
    }
}

/// PBR material with metallic-roughness workflow.
pub struct PbrMaterial {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: RawUniformBuffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: RawUniformBuffer,
    model_bind_group: wgpu::BindGroup,
    pbr_buffer: RawUniformBuffer,
    pbr_bind_group: wgpu::BindGroup,

    /// Base color (albedo).
    pub base_color: [f32; 4],
    /// Metallic factor (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
    /// Roughness factor (0.0 = smooth, 1.0 = rough).
    pub roughness: f32,
    /// Emissive color.
    pub emissive: [f32; 3],
    /// Ambient occlusion factor.
    pub ao: f32,
}

impl PbrMaterial {
    /// Create a new PBR material with default values.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_params(
            ctx,
            format,
            [1.0, 1.0, 1.0, 1.0],
            0.0,
            0.5,
            [0.0, 0.0, 0.0],
            1.0,
        )
    }

    /// Create a new PBR material with custom parameters.
    pub fn with_params(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        emissive: [f32; 3],
        ao: f32,
    ) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/pbr.wgsl");

        // Camera bind group layout (group 0)
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("pbr camera bind group layout"),
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
                    label: Some("pbr model bind group layout"),
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

        // PBR parameters bind group layout (group 2)
        let pbr_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("pbr params bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let pipeline = PipelineBuilder::new(ctx)
            .label("pbr material pipeline")
            .shader(shader)
            .vertex_layout(Vertex::layout())
            .bind_group_layout(&camera_bind_group_layout)
            .bind_group_layout(&model_bind_group_layout)
            .bind_group_layout(&pbr_bind_group_layout)
            .color_format(format)
            .depth(DepthState::read_write())
            .blend(BlendState::Opaque)
            .cull(CullState::Back)
            .build()?;

        // Create camera uniform buffer
        let camera_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<CameraUniform>() as u64,
            Some("pbr camera uniform"),
        );

        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pbr camera bind group"),
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
            Some("pbr model uniform"),
        );

        let model_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pbr model bind group"),
            layout: &model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.buffer().as_entire_binding(),
            }],
        });

        // Create PBR parameters buffer
        let pbr_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<PbrUniform>() as u64,
            Some("pbr params uniform"),
        );

        let pbr_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pbr params bind group"),
            layout: &pbr_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: pbr_buffer.buffer().as_entire_binding(),
            }],
        });

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            model_buffer,
            model_bind_group,
            pbr_buffer,
            pbr_bind_group,
            base_color,
            metallic,
            roughness,
            emissive,
            ao,
        })
    }

    /// Get the PBR bind group.
    pub fn pbr_bind_group(&self) -> &wgpu::BindGroup {
        &self.pbr_bind_group
    }
}

impl Material for PbrMaterial {
    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    fn model_bind_group(&self) -> &wgpu::BindGroup {
        &self.model_bind_group
    }

    fn extra_bind_groups(&self) -> Vec<(u32, &wgpu::BindGroup)> {
        vec![(2, &self.pbr_bind_group)]
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

        let pbr_uniform = PbrUniform {
            base_color: self.base_color,
            emissive: [self.emissive[0], self.emissive[1], self.emissive[2], 0.0],
            metallic: self.metallic,
            roughness: self.roughness,
            ao: self.ao,
            _padding: 0.0,
        };
        self.pbr_buffer.write(ctx, &pbr_uniform);
    }
}
