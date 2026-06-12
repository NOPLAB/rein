//! Fog post-processing effect

use super::{Effect, FullscreenQuad};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{BlendState, CullState};
use crate::core::vertex::VertexPC;
use glam::Vec3;

/// Fog calculation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogMode {
    /// Linear fog (fog = (end - distance) / (end - start)).
    Linear,
    /// Exponential fog (fog = exp(-density * distance)).
    Exponential,
    /// Exponential squared fog (fog = exp(-(density * distance)^2)).
    ExponentialSquared,
}

/// Fog uniform parameters.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FogUniform {
    /// Fog color RGB + mode (0=linear, 1=exp, 2=exp2).
    pub color_mode: [f32; 4],
    /// start, end, density, padding
    pub params: [f32; 4],
}

/// Fog post-processing effect.
pub struct FogEffect {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: RawUniformBuffer,
    sampler: wgpu::Sampler,
    depth_sampler: wgpu::Sampler,
    quad: FullscreenQuad,

    /// Fog color.
    pub color: Vec3,
    /// Fog mode.
    pub mode: FogMode,
    /// Start distance (for linear fog).
    pub start: f32,
    /// End distance (for linear fog).
    pub end: f32,
    /// Density (for exponential fog).
    pub density: f32,
}

impl FogEffect {
    /// Create a new fog effect.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = include_str!("../shaders/effects/fog.wgsl");

        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("fog bind group layout"),
                    entries: &[
                        // Color texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Depth texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Color sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        // Depth sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                        // Fog uniform
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let pipeline = PipelineBuilder::new(ctx)
            .label("fog pipeline")
            .shader(shader)
            .vertex_layout(VertexPC::layout())
            .bind_group_layout(&bind_group_layout)
            .color_format(format)
            .blend(BlendState::Opaque)
            .cull(CullState::None)
            .build()?;

        let uniform_buffer =
            RawUniformBuffer::new(ctx, size_of::<FogUniform>() as u64, Some("fog uniform"));

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fog color sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let depth_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fog depth sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad = FullscreenQuad::new(ctx);

        Ok(Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            sampler,
            depth_sampler,
            quad,
            color: Vec3::new(0.5, 0.6, 0.7),
            mode: FogMode::Linear,
            start: 10.0,
            end: 100.0,
            density: 0.02,
        })
    }

    /// Create a bind group for the fog effect.
    pub fn create_bind_group(
        &self,
        ctx: &WgpuContext,
        color_input: &wgpu::TextureView,
        depth_input: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fog bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color_input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth_input),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.depth_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.uniform_buffer.buffer().as_entire_binding(),
                },
            ],
        })
    }

    /// Update the fog uniform buffer.
    pub fn update_uniform(&self, ctx: &WgpuContext, _near: f32, far: f32) {
        let mode_value = match self.mode {
            FogMode::Linear => 0.0,
            FogMode::Exponential => 1.0,
            FogMode::ExponentialSquared => 2.0,
        };

        let uniform = FogUniform {
            color_mode: [self.color.x, self.color.y, self.color.z, mode_value],
            params: [self.start, self.end, self.density, far],
        };
        self.uniform_buffer.write(ctx, &uniform);
    }

    /// Apply the fog effect with a depth texture.
    pub fn apply_with_depth(
        &self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        color_input: &wgpu::TextureView,
        depth_input: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        let bind_group = self.create_bind_group(ctx, color_input, depth_input);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fog pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        self.quad.draw(&mut render_pass);
    }
}

impl Effect for FogEffect {
    fn apply(
        &self,
        _ctx: &WgpuContext,
        _encoder: &mut wgpu::CommandEncoder,
        _input: &wgpu::TextureView,
        _output: &wgpu::TextureView,
    ) {
        // Fog requires depth texture, so this basic apply doesn't work
        // Use apply_with_depth instead
        panic!("FogEffect requires depth texture. Use apply_with_depth instead.");
    }
}
