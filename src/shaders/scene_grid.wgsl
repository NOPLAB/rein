// Scene ground grid: minor / major lines with distance fade, on an arbitrary plane.

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

struct GridUniform {
    origin: vec4<f32>,       // plane origin (height applied)
    axis_u: vec4<f32>,       // in-plane unit axis
    axis_v: vec4<f32>,       // in-plane unit axis
    params: vec4<f32>,       // step, major_every, half_extent, unused
    minor_color: vec4<f32>,
    major_color: vec4<f32>,
    axis_u_color: vec4<f32>,
    axis_v_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var<uniform> grid: GridUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) plane: vec2<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let e = grid.params.z;
    let c = corners[vi] * e;
    let world = grid.origin.xyz + grid.axis_u.xyz * c.x + grid.axis_v.xyz * c.y;
    var out: VertexOutput;
    out.clip_position = frame.view_proj * vec4<f32>(world, 1.0);
    out.plane = c;
    out.world = world;
    return out;
}

// Anti-aliased line coverage for a coordinate `x` on a grid of `step`, using screen
// derivatives so lines stay ~1.5 px wide at any distance.
fn line_coverage(x: f32, step: f32) -> f32 {
    let d = fwidth(x);
    let f = abs(fract(x / step + 0.5) - 0.5) * step;
    return 1.0 - smoothstep(0.0, d * 1.5, f);
}

fn axis_coverage(x: f32) -> f32 {
    let d = fwidth(x);
    return 1.0 - smoothstep(0.0, d * 2.0, abs(x));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let step = max(grid.params.x, 1e-4);
    let major_every = max(grid.params.y, 1.0);
    let major_step = step * major_every;

    let minor = max(line_coverage(input.plane.x, step), line_coverage(input.plane.y, step));
    let major = max(line_coverage(input.plane.x, major_step), line_coverage(input.plane.y, major_step));
    let au = axis_coverage(input.plane.y); // the u axis is where v == 0
    let av = axis_coverage(input.plane.x);

    var color = grid.minor_color;
    color = mix(color, grid.major_color, major);
    color = mix(color, grid.axis_u_color, au);
    color = mix(color, grid.axis_v_color, av);
    let coverage = max(max(minor * grid.minor_color.a, major * grid.major_color.a),
                       max(au * grid.axis_u_color.a, av * grid.axis_v_color.a));

    // Fade with distance from the eye relative to the extent so the far edge dissolves.
    let dist = length(input.world - frame.eye.xyz);
    let fade = 1.0 - smoothstep(grid.params.z * 0.35, grid.params.z, dist);

    let a = coverage * fade;
    if a <= 0.002 {
        discard;
    }
    return vec4<f32>(color.rgb, a);
}
