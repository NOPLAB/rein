//! Directional light shadow mapping

use super::{ShadowConfig, ShadowMap, ShadowUniform};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{CullState, DepthState};
use crate::core::vertex::VertexP;
use crate::renderer::geometry::Geometry;
use crate::renderer::light::DirectionalLight;
use crate::renderer::Camera;
use glam::{Mat4, Vec3};

/// Shadow uniform for depth pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DepthPassUniform {
    light_view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
}

/// Directional light shadow mapper.
pub struct DirectionalShadow {
    shadow_map: ShadowMap,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: RawUniformBuffer,
    shadow_sampler: wgpu::Sampler,
}

impl DirectionalShadow {
    /// Create a new directional shadow mapper.
    pub fn new(ctx: &WgpuContext, config: ShadowConfig) -> anyhow::Result<Self> {
        let shadow_map = ShadowMap::new(ctx, config);

        let shader = include_str!("../../shaders/shadow.wgsl");

        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("shadow depth bind group layout"),
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
            .label("shadow depth pipeline")
            .shader(shader)
            .vertex_layout(VertexP::layout())
            .bind_group_layout(&bind_group_layout)
            .depth(DepthState::default())
            .cull(CullState::Back)
            .build_depth_only()?;

        let uniform_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<DepthPassUniform>() as u64,
            Some("shadow depth uniform"),
        );

        let shadow_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Ok(Self {
            shadow_map,
            pipeline,
            bind_group_layout,
            uniform_buffer,
            shadow_sampler,
        })
    }

    /// Calculate the light space matrix for a directional light.
    pub fn calculate_light_matrix(
        &mut self,
        light: &DirectionalLight,
        camera: &Camera,
        scene_radius: f32,
    ) {
        // Calculate orthographic projection that covers the scene
        let light_dir = light.direction.normalize();

        // Light view matrix - looking from "infinity" in light direction
        let light_pos = camera.position - light_dir * scene_radius * 2.0;
        let light_view = Mat4::look_at_rh(light_pos, camera.position, Vec3::Y);

        // Orthographic projection that covers the scene
        let ortho_size = scene_radius * 1.5;
        let light_proj = Mat4::orthographic_rh(
            -ortho_size,
            ortho_size,
            -ortho_size,
            ortho_size,
            0.1,
            scene_radius * 4.0,
        );

        self.shadow_map.light_matrix = light_proj * light_view;
    }

    /// Render shadows for a list of objects.
    pub fn render_shadow_pass<G: Geometry>(
        &self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        objects: &[(Mat4, &G)],
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow depth pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.shadow_map.depth_view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.pipeline);

        for (model_matrix, geometry) in objects {
            let uniform = DepthPassUniform {
                light_view_proj: self.shadow_map.light_matrix.to_cols_array_2d(),
                model: model_matrix.to_cols_array_2d(),
            };
            self.uniform_buffer.write(ctx, &uniform);

            let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow depth bind group"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.buffer().as_entire_binding(),
                }],
            });

            render_pass.set_bind_group(0, &bind_group, &[]);
            geometry.draw(&mut render_pass);
        }
    }

    /// Get the shadow map for sampling.
    pub fn shadow_map(&self) -> &ShadowMap {
        &self.shadow_map
    }

    /// Get the shadow sampler for comparison sampling.
    pub fn shadow_sampler(&self) -> &wgpu::Sampler {
        &self.shadow_sampler
    }

    /// Get the shadow uniform data.
    pub fn uniform(&self) -> ShadowUniform {
        self.shadow_map.uniform()
    }

    /// Get the shadow map depth view.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        self.shadow_map.depth_view()
    }

    /// Get the light matrix.
    pub fn light_matrix(&self) -> Mat4 {
        self.shadow_map.light_matrix
    }
}
