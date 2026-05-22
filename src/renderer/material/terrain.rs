//! Terrain material with height-based coloring
//!
//! Provides a material that blends between two colors based on vertex height,
//! with Blinn-Phong lighting.

use super::traits::{Material, ModelUniform};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::{PipelineBuilder, Vertex};
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::renderer::light::Light;
use crate::renderer::viewer::{CameraUniform, Viewer};
use glam::Mat4;

/// Terrain uniform data for GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainUniform {
    pub min_height: f32,
    pub max_height: f32,
    pub padding0: f32,
    pub padding1: f32,
    pub color_low: [f32; 4],
    pub color_high: [f32; 4],
}

/// Material for terrain with height-based coloring.
///
/// Uses a custom shader that blends between a low-altitude color and a high-altitude
/// color based on vertex height.
pub struct TerrainMaterial {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: RawUniformBuffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: RawUniformBuffer,
    model_bind_group: wgpu::BindGroup,
    terrain_buffer: RawUniformBuffer,
    terrain_bind_group: wgpu::BindGroup,

    /// Low altitude color RGBA.
    pub color_low: [f32; 4],
    /// High altitude color RGBA.
    pub color_high: [f32; 4],
    /// Minimum height for color mapping.
    pub min_height: f32,
    /// Maximum height for color mapping.
    pub max_height: f32,
}

impl TerrainMaterial {
    /// Create a new terrain material with default colors.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_params(
            ctx,
            format,
            [0.2, 0.5, 0.1, 1.0],  // green low
            [0.8, 0.8, 0.85, 1.0], // snow high
            0.0,
            10.0,
        )
    }

    /// Create a new terrain material with custom parameters.
    pub fn with_params(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        color_low: [f32; 4],
        color_high: [f32; 4],
        min_height: f32,
        max_height: f32,
    ) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/terrain.wgsl");

        // Camera bind group layout (group 0)
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("terrain camera bind group layout"),
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
                    label: Some("terrain model bind group layout"),
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

        // Terrain bind group layout (group 2)
        let terrain_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("terrain params bind group layout"),
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

        let pipeline = PipelineBuilder::new(ctx)
            .label("terrain material pipeline")
            .shader(shader)
            .vertex_layout(Vertex::layout())
            .bind_group_layout(&camera_bind_group_layout)
            .bind_group_layout(&model_bind_group_layout)
            .bind_group_layout(&terrain_bind_group_layout)
            .color_format(format)
            .depth(DepthState::read_write())
            .blend(BlendState::Opaque)
            .cull(CullState::Back)
            .build()?;

        // Create camera uniform buffer
        let camera_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<CameraUniform>() as u64,
            Some("terrain camera uniform"),
        );

        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain camera bind group"),
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
            Some("terrain model uniform"),
        );

        let model_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain model bind group"),
            layout: &model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.buffer().as_entire_binding(),
            }],
        });

        // Create terrain uniform buffer
        let terrain_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<TerrainUniform>() as u64,
            Some("terrain params uniform"),
        );

        let terrain_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain params bind group"),
            layout: &terrain_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: terrain_buffer.buffer().as_entire_binding(),
            }],
        });

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            model_buffer,
            model_bind_group,
            terrain_buffer,
            terrain_bind_group,
            color_low,
            color_high,
            min_height,
            max_height,
        })
    }
}

impl Material for TerrainMaterial {
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
        vec![(2, &self.terrain_bind_group)]
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

        let terrain_uniform = TerrainUniform {
            min_height: self.min_height,
            max_height: self.max_height,
            padding0: 0.0,
            padding1: 0.0,
            color_low: self.color_low,
            color_high: self.color_high,
        };
        self.terrain_buffer.write(ctx, &terrain_uniform);
    }
}
