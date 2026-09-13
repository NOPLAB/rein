//! Draws a [`Scene`] into any [`RenderTarget`] with shared pipelines.
//!
//! One pipeline per (primitive, blend / depth mode); per-object data lives in a single
//! dynamic-offset uniform buffer, so adding a thousand objects costs no pipeline or bind
//! group creation. Lines and points are re-uploaded every frame (they are the things that
//! change every tick in a telemetry viewer).

use core::mem::size_of;

use glam::{Mat4, Vec3, Vec4};

use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{BlendState, ClearState, CullState, DepthState};
use crate::core::render_target::RenderTarget;
use crate::core::vertex::{VertexPC, VertexPN};
use crate::renderer::viewer::Viewer;

use super::scene::{Layer, Object, PointInstance, Scene, ScreenLabel};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    eye: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    up: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],
    color: [f32; 4],
    emissive: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GridUniform {
    origin: [f32; 4],
    axis_u: [f32; 4],
    axis_v: [f32; 4],
    params: [f32; 4],
    minor_color: [f32; 4],
    major_color: [f32; 4],
    axis_u_color: [f32; 4],
    axis_v_color: [f32; 4],
}

/// A GPU buffer that grows to fit whatever is written each frame.
struct GrowBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl GrowBuffer {
    fn new(
        ctx: &WgpuContext,
        capacity: u64,
        usage: wgpu::BufferUsages,
        label: &'static str,
    ) -> Self {
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity,
            usage,
            label,
        }
    }

    /// Write `bytes`; returns `true` when the buffer had to be re-created.
    fn write(&mut self, ctx: &WgpuContext, bytes: &[u8]) -> bool {
        let needed = bytes.len() as u64;
        let recreated = needed > self.capacity;
        if recreated {
            let capacity = needed.next_power_of_two().max(self.capacity * 2);
            *self = Self::new(ctx, capacity, self.usage, self.label);
        }
        if !bytes.is_empty() {
            ctx.queue.write_buffer(&self.buffer, 0, bytes);
        }
        recreated
    }
}

fn uniform_layout(
    ctx: &WgpuContext,
    label: &str,
    dynamic: bool,
    min_size: u64,
) -> wgpu::BindGroupLayout {
    ctx.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: dynamic,
                    min_binding_size: wgpu::BufferSize::new(min_size),
                },
                count: None,
            }],
        })
}

fn bind_uniform(
    ctx: &WgpuContext,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    size: Option<u64>,
) -> wgpu::BindGroup {
    ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: size.and_then(wgpu::BufferSize::new),
            }),
        }],
    })
}

/// How one [`SceneRenderer::render_with`] call uses the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Clear colour and depth to the scene background first. `false` draws over whatever
    /// is already in the target (a second scene composited on top).
    pub clear: bool,
    /// Restrict drawing to a pixel rectangle `[x, y, width, height]` of the target. The
    /// depth buffer inside the rectangle is reset, so the region behaves like a small
    /// independent viewport (an axis gizmo in a corner, a picture-in-picture view).
    pub region: Option<[u32; 4]>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            clear: true,
            region: None,
        }
    }
}

/// A prepared draw of one object.
struct ObjectDraw<'a> {
    object: &'a Object,
    offset: u32,
    /// Distance from the eye (for back-to-front sorting).
    depth: f32,
}

/// Renders [`Scene`]s.
pub struct SceneRenderer {
    format: wgpu::TextureFormat,
    frame_buffer: RawUniformBuffer,
    frame_bind_group: wgpu::BindGroup,
    object_layout: wgpu::BindGroupLayout,
    object_buffer: GrowBuffer,
    object_bind_group: wgpu::BindGroup,
    object_stride: u64,
    grid_buffer: RawUniformBuffer,
    grid_bind_group: wgpu::BindGroup,
    line_buffer: GrowBuffer,
    point_buffer: GrowBuffer,
    mesh_opaque: wgpu::RenderPipeline,
    mesh_blend: wgpu::RenderPipeline,
    mesh_overlay: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    line_overlay: wgpu::RenderPipeline,
    point: wgpu::RenderPipeline,
    point_overlay: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    /// Writes far depth over the current viewport without touching colour.
    depth_reset: wgpu::RenderPipeline,
}

impl SceneRenderer {
    /// Build the pipelines for a colour target of `format`.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let frame_layout =
            uniform_layout(ctx, "scene frame", false, size_of::<FrameUniform>() as u64);
        let frame_buffer =
            RawUniformBuffer::new(ctx, size_of::<FrameUniform>() as u64, Some("scene frame"));
        let frame_bind_group = bind_uniform(ctx, &frame_layout, frame_buffer.buffer(), None);

        let align = u64::from(ctx.device.limits().min_uniform_buffer_offset_alignment);
        let object_stride = (size_of::<ObjectUniform>() as u64).div_ceil(align) * align;
        let object_layout =
            uniform_layout(ctx, "scene object", true, size_of::<ObjectUniform>() as u64);
        let object_buffer = GrowBuffer::new(
            ctx,
            object_stride * 256,
            wgpu::BufferUsages::UNIFORM,
            "scene objects",
        );
        let object_bind_group = bind_uniform(
            ctx,
            &object_layout,
            &object_buffer.buffer,
            Some(size_of::<ObjectUniform>() as u64),
        );

        let grid_layout = uniform_layout(ctx, "scene grid", false, size_of::<GridUniform>() as u64);
        let grid_buffer =
            RawUniformBuffer::new(ctx, size_of::<GridUniform>() as u64, Some("scene grid"));
        let grid_bind_group = bind_uniform(ctx, &grid_layout, grid_buffer.buffer(), None);

        let line_buffer =
            GrowBuffer::new(ctx, 64 * 1024, wgpu::BufferUsages::VERTEX, "scene lines");
        let point_buffer =
            GrowBuffer::new(ctx, 64 * 1024, wgpu::BufferUsages::VERTEX, "scene points");

        let mesh_shader = include_str!("../shaders/scene_mesh.wgsl");
        let mesh = |label, depth, blend| {
            PipelineBuilder::new(ctx)
                .label(label)
                .shader(mesh_shader)
                .vertex_layout(VertexPN::layout())
                .bind_group_layout(&frame_layout)
                .bind_group_layout(&object_layout)
                .color_format(format)
                .depth(depth)
                .blend(blend)
                .cull(CullState::None)
                .build()
        };
        let mesh_opaque = mesh(
            "scene mesh opaque",
            DepthState::read_write(),
            BlendState::Opaque,
        )?;
        let mesh_blend = mesh(
            "scene mesh blend",
            DepthState::read_only(),
            BlendState::Alpha,
        )?;
        let mesh_overlay = mesh(
            "scene mesh overlay",
            DepthState::disabled(),
            BlendState::Alpha,
        )?;

        let line_shader = include_str!("../shaders/scene_line.wgsl");
        let line_pipe = |label, depth| {
            PipelineBuilder::new(ctx)
                .label(label)
                .shader(line_shader)
                .vertex_layout(VertexPC::layout())
                .bind_group_layout(&frame_layout)
                .color_format(format)
                .depth(depth)
                .blend(BlendState::Alpha)
                .cull(CullState::None)
                .topology(wgpu::PrimitiveTopology::LineList)
                .build()
        };
        let line = line_pipe("scene lines", DepthState::read_write())?;
        let line_overlay = line_pipe("scene lines overlay", DepthState::disabled())?;

        let point_shader = include_str!("../shaders/scene_point.wgsl");
        let point_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<PointInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4],
        };
        let point_pipe = |label, depth| {
            PipelineBuilder::new(ctx)
                .label(label)
                .shader(point_shader)
                .vertex_layout(point_layout.clone())
                .bind_group_layout(&frame_layout)
                .color_format(format)
                .depth(depth)
                .blend(BlendState::Alpha)
                .cull(CullState::None)
                .build()
        };
        let point = point_pipe("scene points", DepthState::read_write())?;
        let point_overlay = point_pipe("scene points overlay", DepthState::disabled())?;

        let grid = PipelineBuilder::new(ctx)
            .label("scene grid")
            .shader(include_str!("../shaders/scene_grid.wgsl"))
            .bind_group_layout(&frame_layout)
            .bind_group_layout(&grid_layout)
            .color_format(format)
            .depth(DepthState {
                write: false,
                compare: wgpu::CompareFunction::LessEqual,
            })
            .depth_bias(-2, -1.0)
            .blend(BlendState::Alpha)
            .cull(CullState::None)
            .build()?;

        let depth_reset = PipelineBuilder::new(ctx)
            .label("scene depth reset")
            .shader(include_str!("../shaders/scene_depth_reset.wgsl"))
            .color_format(format)
            .depth(DepthState {
                write: true,
                compare: wgpu::CompareFunction::Always,
            })
            // Colour writes are neutralised by the blend state: destination stays as is.
            .blend(BlendState::KeepDestination)
            .cull(CullState::None)
            .build()?;

        Ok(Self {
            format,
            frame_buffer,
            frame_bind_group,
            object_layout,
            object_buffer,
            object_bind_group,
            object_stride,
            grid_buffer,
            grid_bind_group,
            line_buffer,
            point_buffer,
            mesh_opaque,
            mesh_blend,
            mesh_overlay,
            line,
            line_overlay,
            point,
            point_overlay,
            grid,
            depth_reset,
        })
    }

    /// The colour format the pipelines were built for.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Draw `scene` from `viewer` into `target`, clearing it to the scene background.
    /// Returns the projected labels for a text renderer to draw on top.
    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        target: &RenderTarget<'_>,
        viewer: &dyn Viewer,
        scene: &Scene,
    ) -> Vec<ScreenLabel> {
        self.render_with(ctx, target, viewer, scene, RenderOptions::default())
    }

    /// [`Self::render`] with explicit clear / region options. Label positions are in
    /// target pixels (the region offset is already applied).
    pub fn render_with(
        &mut self,
        ctx: &WgpuContext,
        target: &RenderTarget<'_>,
        viewer: &dyn Viewer,
        scene: &Scene,
        options: RenderOptions,
    ) -> Vec<ScreenLabel> {
        let region = options
            .region
            .unwrap_or_else(|| [0, 0, target.width(), target.height()]);
        let (ox, oy) = (region[0] as f32, region[1] as f32);
        let (width, height) = (region[2].max(1) as f32, region[3].max(1) as f32);
        let view = viewer.view_matrix();
        let proj = viewer.projection_matrix();
        let view_proj = proj * view;
        let eye = viewer.position();
        let inv_view = view.inverse();
        let cam_right = inv_view.x_axis.truncate().normalize_or(Vec3::X);
        let cam_up = inv_view.y_axis.truncate().normalize_or(Vec3::Y);
        let up = scene.up.normalize_or(Vec3::Y);
        let l = &scene.lighting;
        let sun_dir = (-l.sun_direction).normalize_or(up);
        let v4 = |v: Vec3| [v.x, v.y, v.z, 0.0];
        let c4 = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        self.frame_buffer.write(
            ctx,
            &FrameUniform {
                view_proj: view_proj.to_cols_array_2d(),
                view: view.to_cols_array_2d(),
                eye: [eye.x, eye.y, eye.z, 1.0],
                sun_dir: v4(sun_dir),
                sun_color: c4(l.sun_color),
                sky: c4(l.sky),
                ground: c4(l.ground),
                up: v4(up),
                cam_right: v4(cam_right),
                cam_up: v4(cam_up),
                viewport: [width, height, 0.0, 0.0],
            },
        );

        // ----- objects -------------------------------------------------------------
        let stride = self.object_stride as usize;
        let mut draws: Vec<ObjectDraw<'_>> = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        for (_, o) in scene.objects() {
            if !o.visible {
                continue;
            }
            let offset = bytes.len();
            bytes.resize(offset + stride, 0);
            let u = ObjectUniform {
                model: o.transform.to_cols_array_2d(),
                normal_matrix: o.transform.inverse().transpose().to_cols_array_2d(),
                color: o.surface.color,
                emissive: [
                    o.surface.emissive[0],
                    o.surface.emissive[1],
                    o.surface.emissive[2],
                    if o.surface.unlit { 1.0 } else { 0.0 },
                ],
            };
            bytes[offset..offset + size_of::<ObjectUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&u));
            let centre = o.world_aabb().center();
            draws.push(ObjectDraw {
                object: o,
                offset: offset as u32,
                depth: (centre - eye).length(),
            });
        }
        if self.object_buffer.write(ctx, &bytes) {
            self.object_bind_group = bind_uniform(
                ctx,
                &self.object_layout,
                &self.object_buffer.buffer,
                Some(size_of::<ObjectUniform>() as u64),
            );
        }

        // ----- lines / points --------------------------------------------------------
        let mut line_verts: Vec<VertexPC> = Vec::new();
        let mut line_ranges = [(0_u32, 0_u32); 2]; // [scene, overlay]
        for overlay in [false, true] {
            let start = line_verts.len() as u32;
            for set in scene.lines.iter().filter(|s| s.visible) {
                if (set.layer == Layer::Overlay) == overlay {
                    line_verts.extend(set.world_segments());
                }
            }
            line_ranges[usize::from(overlay)] = (start, line_verts.len() as u32);
        }
        let _ = self
            .line_buffer
            .write(ctx, bytemuck::cast_slice(&line_verts));

        let mut point_insts: Vec<PointInstance> = Vec::new();
        let mut point_ranges = [(0_u32, 0_u32); 2];
        for overlay in [false, true] {
            let start = point_insts.len() as u32;
            for set in scene.points.iter().filter(|s| s.visible) {
                if (set.layer == Layer::Overlay) == overlay {
                    point_insts.extend(set.world_points());
                }
            }
            point_ranges[usize::from(overlay)] = (start, point_insts.len() as u32);
        }
        let _ = self
            .point_buffer
            .write(ctx, bytemuck::cast_slice(&point_insts));

        // ----- grid ------------------------------------------------------------------
        if let Some(g) = &scene.grid {
            let axis_u = up.any_orthonormal_vector();
            let axis_v = up.cross(axis_u).normalize_or(Vec3::Y);
            // Prefer world X as the first axis when it is in-plane (Z-up / Y-up scenes).
            let (axis_u, axis_v) = if up.dot(Vec3::X).abs() < 1e-6 {
                (Vec3::X, up.cross(Vec3::X).normalize_or(axis_v))
            } else {
                (axis_u, axis_v)
            };
            let extent = if g.step > 0.0 {
                (g.half_extent / g.step).ceil() * g.step
            } else {
                g.half_extent
            };
            let origin = up * g.height;
            self.grid_buffer.write(
                ctx,
                &GridUniform {
                    origin: [origin.x, origin.y, origin.z, 1.0],
                    axis_u: v4(axis_u),
                    axis_v: v4(axis_v),
                    params: [g.step, g.major_every.max(1) as f32, extent, 0.0],
                    minor_color: g.minor_color,
                    major_color: g.major_color,
                    axis_u_color: g.axis_u_color,
                    axis_v_color: g.axis_v_color,
                },
            );
        }

        // ----- draw ------------------------------------------------------------------
        let (opaque, rest): (Vec<_>, Vec<_>) = draws
            .iter()
            .partition(|d| d.object.layer != Layer::Overlay && !d.object.surface.is_transparent());
        let (mut blend, overlay): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|d| d.object.layer != Layer::Overlay);
        blend.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        let mut ground: Vec<&ObjectDraw<'_>> = opaque
            .iter()
            .copied()
            .filter(|d| d.object.layer == Layer::Ground)
            .collect();
        ground.extend(
            opaque
                .iter()
                .copied()
                .filter(|d| d.object.layer != Layer::Ground),
        );

        let mut encoder = ctx.create_encoder(Some("scene"));
        {
            let clear = if options.clear {
                ClearState::color_and_depth(scene.background, 1.0)
            } else {
                ClearState::none()
            };
            let mut pass = target.begin_render_pass(&mut encoder, clear);
            if let Some([x, y, w, h]) = options.region {
                let w = w.min(target.width().saturating_sub(x)).max(1);
                let h = h.min(target.height().saturating_sub(y)).max(1);
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w, h);
                pass.set_pipeline(&self.depth_reset);
                pass.draw(0..3, 0..1);
            }
            pass.set_bind_group(0, &self.frame_bind_group, &[]);

            let draw_objects = |pass: &mut wgpu::RenderPass<'_>, list: &[&ObjectDraw<'_>]| {
                for d in list {
                    pass.set_bind_group(1, &self.object_bind_group, &[d.offset]);
                    let mesh = &d.object.mesh;
                    pass.set_vertex_buffer(0, mesh.vertex_buffer().slice());
                    let ib = mesh.index_buffer();
                    pass.set_index_buffer(ib.slice(), ib.format());
                    pass.draw_indexed(0..ib.count(), 0, 0..1);
                }
            };

            pass.set_pipeline(&self.mesh_opaque);
            draw_objects(&mut pass, &ground);

            // The grid goes over opaque geometry: its pipeline compares `LessEqual` with a
            // depth bias, so a ground plate lying exactly on the grid plane does not hide it.
            if scene.grid.is_some() {
                pass.set_pipeline(&self.grid);
                pass.set_bind_group(1, &self.grid_bind_group, &[]);
                pass.draw(0..6, 0..1);
            }

            let stride_line = size_of::<VertexPC>() as u64;
            let stride_point = size_of::<PointInstance>() as u64;
            let (ls, le) = line_ranges[0];
            if le > ls {
                pass.set_pipeline(&self.line);
                pass.set_vertex_buffer(
                    0,
                    self.line_buffer
                        .buffer
                        .slice(u64::from(ls) * stride_line..u64::from(le) * stride_line),
                );
                pass.draw(0..(le - ls), 0..1);
            }
            let (ps, pe) = point_ranges[0];
            if pe > ps {
                pass.set_pipeline(&self.point);
                pass.set_vertex_buffer(
                    0,
                    self.point_buffer
                        .buffer
                        .slice(u64::from(ps) * stride_point..u64::from(pe) * stride_point),
                );
                pass.draw(0..6, 0..(pe - ps));
            }

            if !blend.is_empty() {
                pass.set_pipeline(&self.mesh_blend);
                draw_objects(&mut pass, &blend);
            }

            if !overlay.is_empty() {
                pass.set_pipeline(&self.mesh_overlay);
                draw_objects(&mut pass, &overlay);
            }
            let (ls, le) = line_ranges[1];
            if le > ls {
                pass.set_pipeline(&self.line_overlay);
                pass.set_vertex_buffer(
                    0,
                    self.line_buffer
                        .buffer
                        .slice(u64::from(ls) * stride_line..u64::from(le) * stride_line),
                );
                pass.draw(0..(le - ls), 0..1);
            }
            let (ps, pe) = point_ranges[1];
            if pe > ps {
                pass.set_pipeline(&self.point_overlay);
                pass.set_vertex_buffer(
                    0,
                    self.point_buffer
                        .buffer
                        .slice(u64::from(ps) * stride_point..u64::from(pe) * stride_point),
                );
                pass.draw(0..6, 0..(pe - ps));
            }
        }
        ctx.submit(Some(encoder.finish()));

        // ----- labels ----------------------------------------------------------------
        let py = proj.y_axis.y.abs();
        scene
            .labels
            .iter()
            .filter_map(|l| {
                let clip = view_proj * l.position.extend(1.0);
                if clip.w <= 0.0 {
                    return None;
                }
                let ndc = clip.truncate() / clip.w;
                if !(-1.0..=1.0).contains(&ndc.z) {
                    return None;
                }
                let size = if l.height > 0.0 {
                    l.height * py * height / (2.0 * clip.w)
                } else {
                    l.pixel_size
                };
                Some(ScreenLabel {
                    x: ox + (ndc.x + 1.0) * 0.5 * width,
                    y: oy + (1.0 - ndc.y) * 0.5 * height,
                    size,
                    text: l.text.clone(),
                    color: l.color,
                    depth: ndc.z,
                })
            })
            .collect()
    }
}

/// Distance helper shared with tests: view-space depth of a world point.
#[allow(dead_code, reason = "kept for symmetry with the shader; used by tests")]
fn view_depth(view: Mat4, p: Vec3) -> f32 {
    -(view * Vec4::from((p, 1.0))).z
}
