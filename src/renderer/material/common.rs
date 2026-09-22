//! Shared construction helpers for material uniform bindings.

use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;

pub(super) fn uniform_layout(
    ctx: &WgpuContext,
    label: &'static str,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    ctx.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
}

pub(super) fn uniform_binding(
    ctx: &WgpuContext,
    layout: &wgpu::BindGroupLayout,
    size: u64,
    label: &'static str,
) -> (RawUniformBuffer, wgpu::BindGroup) {
    let buffer = RawUniformBuffer::new(ctx, size, Some(label));
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.buffer().as_entire_binding(),
        }],
    });
    (buffer, bind_group)
}
