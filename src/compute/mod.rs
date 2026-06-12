//! Compute shader dispatch utilities.
//!
//! Provides `ComputeDispatcher` for dispatching compute shaders and
//! readback helpers for GPU-to-CPU data transfer.

use crate::context::WgpuContext;
use crate::core::StorageBuffer;

/// Helper for dispatching compute shader workloads.
///
/// Wraps the boilerplate of creating a command encoder, beginning a compute
/// pass, setting pipeline/bind groups, dispatching workgroups, and submitting.
pub struct ComputeDispatcher<'a> {
    ctx: &'a WgpuContext,
}

impl<'a> ComputeDispatcher<'a> {
    /// Create a new compute dispatcher.
    pub fn new(ctx: &'a WgpuContext) -> Self {
        Self { ctx }
    }

    /// Dispatch a single compute pass.
    ///
    /// Sets the pipeline, binds all bind groups in order, dispatches workgroups,
    /// and submits the command buffer.
    pub fn dispatch(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind_groups: &[&wgpu::BindGroup],
        workgroups: [u32; 3],
        label: Option<&str>,
    ) {
        let mut encoder = self.ctx.create_encoder(label);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label,
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            for (i, bg) in bind_groups.iter().enumerate() {
                pass.set_bind_group(i as u32, *bg, &[]);
            }
            pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
        }
        self.ctx.submit([encoder.finish()]);
    }

    /// Dispatch a 1D compute workload.
    ///
    /// Automatically calculates the number of workgroups needed to cover
    /// `total_invocations` with the given `workgroup_size`.
    pub fn dispatch_1d(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind_groups: &[&wgpu::BindGroup],
        total_invocations: u32,
        workgroup_size: u32,
        label: Option<&str>,
    ) {
        let workgroups_x = compute_workgroup_count(total_invocations, workgroup_size);
        self.dispatch(pipeline, bind_groups, [workgroups_x, 1, 1], label);
    }
}

/// Calculate the number of workgroups needed to cover `total_items`
/// with a given `workgroup_size`. Rounds up.
pub fn compute_workgroup_count(total_items: u32, workgroup_size: u32) -> u32 {
    total_items.div_ceil(workgroup_size)
}

/// Read data back from a raw `wgpu::Buffer` to the CPU synchronously.
///
/// Creates a staging buffer, copies data from the source buffer, maps it,
/// and returns the result as a `Vec<T>`. Blocks until the data is available.
///
/// # Panics
///
/// Panics if the buffer mapping fails or the device is lost.
pub fn read_buffer_sync<T: bytemuck::Pod>(
    ctx: &WgpuContext,
    source: &wgpu::Buffer,
    size: u64,
) -> Vec<T> {
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging_readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = ctx.create_encoder(Some("readback copy"));
    encoder.copy_buffer_to_buffer(source, 0, &staging, 0, size);
    ctx.submit([encoder.finish()]);

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result)
            .expect("readback channel receiver dropped before map_async completed");
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()
        .expect("map_async callback never ran")
        .expect("Failed to map staging buffer");

    let data = slice.get_mapped_range();
    let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging.unmap();

    result
}

/// Read data back from a `StorageBuffer` to the CPU synchronously.
///
/// Convenience wrapper around [`read_buffer_sync`] for `StorageBuffer`.
pub fn read_back<T: bytemuck::Pod>(ctx: &WgpuContext, buffer: &StorageBuffer) -> Vec<T> {
    read_buffer_sync(ctx, buffer.buffer(), buffer.size())
}

/// Async version of [`read_back`]. Reads data from a `StorageBuffer` to the CPU.
#[expect(
    clippy::unused_async,
    reason = "kept async to preserve the public API; the body currently uses blocking poll"
)]
pub async fn read_back_async<T: bytemuck::Pod>(
    ctx: &WgpuContext,
    buffer: &StorageBuffer,
) -> Vec<T> {
    let size = buffer.size();

    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("compute readback staging"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = ctx.create_encoder(Some("compute readback"));
    encoder.copy_buffer_to_buffer(buffer.buffer(), 0, &staging, 0, size);
    ctx.submit([encoder.finish()]);

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result)
            .expect("readback channel receiver dropped before map_async completed");
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()
        .expect("map_async callback never ran")
        .expect("Failed to map staging buffer");

    let data = slice.get_mapped_range();
    let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging.unmap();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_workgroup_count() {
        assert_eq!(compute_workgroup_count(100, 64), 2);
        assert_eq!(compute_workgroup_count(128, 64), 2);
        assert_eq!(compute_workgroup_count(129, 64), 3);
        assert_eq!(compute_workgroup_count(1, 64), 1);
        assert_eq!(compute_workgroup_count(0, 64), 0);
        assert_eq!(compute_workgroup_count(64, 64), 1);
    }
}
