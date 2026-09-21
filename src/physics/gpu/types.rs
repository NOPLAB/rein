//! CPU-side data layouts shared with the GPU physics shaders.

/// GPU AABB data layout matching the broadphase shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuAabb {
    pub min: [f32; 3],
    pub entity_id: u32,
    pub max: [f32; 3],
    pub padding: u32,
}

/// GPU collision pair output.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CollisionPair {
    pub entity_a: u32,
    pub entity_b: u32,
}

/// GPU body data layout matching the integrate shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuBody {
    pub position: [f32; 3],
    pub body_type: u32,
    pub linear_velocity: [f32; 3],
    pub mass: f32,
    pub angular_velocity: [f32; 3],
    pub gravity_scale: f32,
    pub force_accumulator: [f32; 3],
    pub linear_damping: f32,
    pub torque_accumulator: [f32; 3],
    pub angular_damping: f32,
    pub inertia_diag: [f32; 3],
    pub padding: f32,
    pub rotation: [f32; 4],
}

/// GPU broadphase parameters.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct BroadphaseParams {
    pub(super) num_bodies: u32,
    pub(super) max_pairs: u32,
    /// Cell size as f32 bits (bitcast<f32> in shader).
    pub(super) cell_size_bits: u32,
    pub(super) _pad0: u32,
}

/// GPU shape data for narrowphase.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuShapeData {
    pub position: [f32; 3],
    pub shape_type: u32, // 0=sphere, 1=box
    pub data: [f32; 4],  // sphere: [radius,0,0,0], box: [hx,hy,hz,0]
    pub axis_x: [f32; 3],
    pub scale_x: f32,
    pub axis_y: [f32; 3],
    pub scale_y: f32,
    pub axis_z: [f32; 3],
    pub scale_z: f32,
}

/// GPU narrowphase result.
///
/// Layout must match the WGSL struct with 16-byte aligned `vec3<f32>` fields.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NarrowphaseResult {
    pub entity_a: u32,
    pub entity_b: u32,
    pub pad0: u32,
    pub pad1: u32,
    pub normal: [f32; 3],
    pub penetration: f32,
    pub point: [f32; 3],
    pub has_contact: u32,
}

/// GPU integrate parameters.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct IntegrateParams {
    pub(super) num_bodies: u32,
    pub(super) dt: f32,
    pub(super) gravity_x: f32,
    pub(super) gravity_y: f32,
    pub(super) gravity_z: f32,
    pub(super) _pad0: f32,
    pub(super) _pad1: f32,
    pub(super) _pad2: f32,
}
