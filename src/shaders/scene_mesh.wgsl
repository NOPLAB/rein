// Scene mesh shader: hemisphere ambient + one directional light, per-object colour.

struct FrameUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    eye: vec4<f32>,
    sun_dir: vec4<f32>,      // unit vector *towards* the light
    sun_color: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    up: vec4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    viewport: vec4<f32>,     // width, height, near, far
};

struct ObjectUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    color: vec4<f32>,
    emissive: vec4<f32>,     // rgb emissive, w = 1 → unlit
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var<uniform> object: ObjectUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = object.model * vec4<f32>(input.position, 1.0);
    out.clip_position = frame.view_proj * world;
    out.world_position = world.xyz;
    out.world_normal = (object.normal_matrix * vec4<f32>(input.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(input: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let base = object.color;
    if object.emissive.w > 0.5 {
        return vec4<f32>(base.rgb + object.emissive.rgb, base.a);
    }
    var n = normalize(input.world_normal);
    if !front {
        n = -n;
    }
    // Hemisphere ambient by the normal's alignment with `up`.
    let h = dot(n, frame.up.xyz) * 0.5 + 0.5;
    let ambient = mix(frame.ground.rgb, frame.sky.rgb, h);
    let diffuse = max(dot(n, frame.sun_dir.xyz), 0.0) * frame.sun_color.rgb;
    let view_dir = normalize(frame.eye.xyz - input.world_position);
    let half_dir = normalize(frame.sun_dir.xyz + view_dir);
    let specular = pow(max(dot(n, half_dir), 0.0), 24.0) * 0.15 * frame.sun_color.rgb;
    let lit = base.rgb * (ambient + diffuse) + specular + object.emissive.rgb;
    return vec4<f32>(lit, base.a);
}
