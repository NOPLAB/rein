//! Grid material for rendering ground grids

use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::{PipelineBuilder, Vertex};
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::renderer::viewer::{CameraUniform, Viewer};

/// Grid material for rendering a ground grid.
pub struct GridMaterial {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: RawUniformBuffer,
    camera_bind_group: wgpu::BindGroup,
}

impl GridMaterial {
    /// Create a new grid material.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/grid.wgsl");

        // Camera bind group layout (group 0)
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("grid camera bind group layout"),
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
            .label("grid material pipeline")
            .shader(shader)
            .vertex_entry("vs_grid")
            .fragment_entry("fs_grid")
            .vertex_layout(Vertex::layout())
            .bind_group_layout(&camera_bind_group_layout)
            .color_format(format)
            .depth(DepthState::read_write())
            .blend(BlendState::Alpha)
            .cull(CullState::None)
            .build()?;

        // Create camera uniform buffer
        let camera_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<CameraUniform>() as u64,
            Some("grid camera uniform"),
        );

        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.buffer().as_entire_binding(),
            }],
        });

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
        })
    }

    /// Update camera uniform.
    pub fn update_camera(&self, ctx: &WgpuContext, viewer: &dyn Viewer) {
        let camera_uniform = CameraUniform::from_viewer(viewer);
        self.camera_buffer.write(ctx, &camera_uniform);
    }

    /// Get the pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get the camera bind group.
    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }
}
