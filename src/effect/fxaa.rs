//! FXAA (Fast Approximate Anti-Aliasing) effect

use super::{Effect, FullscreenQuad};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{BlendState, CullState};
use crate::core::vertex::VertexPC;

/// FXAA uniform parameters.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FxaaUniform {
    /// Texture size (width, height, 1/width, 1/height).
    pub texture_size: [f32; 4],
}

/// FXAA post-processing effect for anti-aliasing.
pub struct FxaaEffect {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: RawUniformBuffer,
    sampler: wgpu::Sampler,
    quad: FullscreenQuad,
}

impl FxaaEffect {
    /// Create a new FXAA effect.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = include_str!("../shaders/effects/fxaa.wgsl");

        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("fxaa bind group layout"),
                    entries: &[
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
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
            .label("fxaa pipeline")
            .shader(shader)
            .vertex_layout(VertexPC::layout())
            .bind_group_layout(&bind_group_layout)
            .color_format(format)
            .blend(BlendState::Opaque)
            .cull(CullState::None)
            .build()?;

        let uniform_buffer =
            RawUniformBuffer::new(ctx, size_of::<FxaaUniform>() as u64, Some("fxaa uniform"));

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fxaa sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let quad = FullscreenQuad::new(ctx);

        Ok(Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            sampler,
            quad,
        })
    }

    fn create_bind_group(&self, ctx: &WgpuContext, input: &wgpu::TextureView) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fxaa bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.buffer().as_entire_binding(),
                },
            ],
        })
    }

    /// Update the texture size uniform.
    pub fn update_size(&self, ctx: &WgpuContext, width: u32, height: u32) {
        let uniform = FxaaUniform {
            texture_size: [
                width as f32,
                height as f32,
                1.0 / width as f32,
                1.0 / height as f32,
            ],
        };
        self.uniform_buffer.write(ctx, &uniform);
    }
}

impl Effect for FxaaEffect {
    fn apply(
        &self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        let bind_group = self.create_bind_group(ctx, input);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fxaa pass"),
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
