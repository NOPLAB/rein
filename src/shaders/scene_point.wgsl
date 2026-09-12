// Scene point shader: instanced camera-facing discs.

struct FrameUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    eye: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    up: vec4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct InstanceInput {
    @location(0) position: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: InstanceInput) -> VertexOutput {
    // Two triangles over a unit quad: (-1,-1) (1,-1) (1,1) / (-1,-1) (1,1) (-1,1).
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[vi];
    let half = inst.size * 0.5;
    let world = inst.position + frame.cam_right.xyz * (c.x * half) + frame.cam_up.xyz * (c.y * half);
    var out: VertexOutput;
    out.clip_position = frame.view_proj * vec4<f32>(world, 1.0);
    out.color = inst.color;
    out.uv = c;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let r2 = dot(input.uv, input.uv);
    if r2 > 1.0 {
        discard;
    }
    // Soft edge.
    let a = input.color.a * (1.0 - smoothstep(0.8, 1.0, r2));
    return vec4<f32>(input.color.rgb, a);
}
