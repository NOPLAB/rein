//! Primitive 2D rendering
//!
//! Provides rendering for 2D primitives like rectangles and circles.

use crate::context::WgpuContext;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Vertex structure for 2D primitives.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub mode: u32,
}

/// A contiguous run of vertices sharing one scissor (clip) rectangle.
#[derive(Clone, Copy)]
struct DrawBatch {
    /// First vertex index covered by this batch.
    start: u32,
    /// Clip rectangle `[x, y, w, h]` in physical pixels, or `None` for no clip.
    scissor: Option<[u32; 4]>,
}

/// Renderer for 2D primitives.
pub struct PrimitiveRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
    screen_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    screen_size: [f32; 2],
    // Scissor batching: each batch records where it starts and which clip applies.
    batches: Vec<DrawBatch>,
    clip_stack: Vec<[u32; 4]>,
}

impl PrimitiveRenderer {
    /// Create a new primitive renderer.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> Self {
        let shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("GUI Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gui.wgsl").into()),
            });

        let vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GUI Vertex Buffer"),
            size: 1024 * size_of::<Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let screen_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GUI Screen Buffer"),
                contents: bytemuck::cast_slice(&[0.0_f32; 4]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("GUI Bind Group Layout"),
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

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GUI Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("GUI Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                immediate_size: 0,
            });

        let pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("GUI Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 32,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Uint32,
                            },
                        ],
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        Self {
            pipeline,
            vertex_buffer,
            vertices: Vec::new(),
            screen_buffer,
            bind_group,
            screen_size: [0.0, 0.0],
            batches: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    /// Add a rectangle to the draw list.
    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.push_quad(x, y, w, h, [0.0, 0.0], [1.0, 1.0], color, 0);
    }

    /// Add a circle to the draw list.
    pub fn draw_circle(&mut self, x: f32, y: f32, radius: f32, color: [f32; 4]) {
        self.push_quad(
            x,
            y,
            radius * 2.0,
            radius * 2.0,
            [0.0, 0.0],
            [1.0, 1.0],
            color,
            1,
        );
    }

    /// Add a straight line segment of the given thickness to the draw list.
    pub fn draw_line(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = dx.hypot(dy);
        if len <= f32::EPSILON {
            return;
        }
        // Unit normal scaled to half thickness, used to offset the segment into a quad.
        let half = thickness * 0.5;
        let (nx, ny) = (-dy / len * half, dx / len * half);
        self.push_quad_raw(
            [x0 + nx, y0 + ny],
            [x1 + nx, y1 + ny],
            [x1 - nx, y1 - ny],
            [x0 - nx, y0 - ny],
            color,
        );
    }

    /// Add a connected polyline (no miter joins — corners may show a small gap).
    pub fn draw_polyline(&mut self, points: &[[f32; 2]], thickness: f32, color: [f32; 4]) {
        for pair in points.windows(2) {
            self.draw_line(
                pair[0][0], pair[0][1], pair[1][0], pair[1][1], thickness, color,
            );
        }
    }

    /// Push a clip rectangle. Drawing is scissored to the intersection of this
    /// rectangle with any clip already on the stack. Balance every call with
    /// [`pop_clip`](Self::pop_clip).
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut x0 = x.max(0.0);
        let mut y0 = y.max(0.0);
        let mut x1 = (x + w).max(x0);
        let mut y1 = (y + h).max(y0);
        if let Some(&[px, py, pw, ph]) = self.clip_stack.last() {
            x0 = x0.max(px as f32);
            y0 = y0.max(py as f32);
            x1 = x1.min((px + pw) as f32);
            y1 = y1.min((py + ph) as f32);
        }
        self.clip_stack.push([
            x0 as u32,
            y0 as u32,
            (x1 - x0).max(0.0) as u32,
            (y1 - y0).max(0.0) as u32,
        ]);
    }

    /// Pop the most recently pushed clip rectangle.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Start a new batch if the active clip differs from the open batch's clip.
    fn begin_geometry(&mut self) {
        let scissor = self.clip_stack.last().copied();
        let start = self.vertices.len() as u32;
        match self.batches.last() {
            Some(b) if b.scissor == scissor => {}
            _ => self.batches.push(DrawBatch { start, scissor }),
        }
    }

    /// Push an arbitrary (possibly rotated) quad from four corners, mode 0 (solid).
    fn push_quad_raw(
        &mut self,
        p0: [f32; 2],
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
    ) {
        self.begin_geometry();
        let v = |position: [f32; 2]| Vertex {
            position,
            uv: [0.0, 0.0],
            color,
            mode: 0,
        };
        self.vertices
            .extend_from_slice(&[v(p0), v(p1), v(p2), v(p0), v(p2), v(p3)]);
    }

    fn push_quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color: [f32; 4],
        mode: u32,
    ) {
        self.begin_geometry();
        let v0 = Vertex {
            position: [x, y],
            uv: [uv_min[0], uv_min[1]],
            color,
            mode,
        };
        let v1 = Vertex {
            position: [x, y + h],
            uv: [uv_min[0], uv_max[1]],
            color,
            mode,
        };
        let v2 = Vertex {
            position: [x + w, y + h],
            uv: [uv_max[0], uv_max[1]],
            color,
            mode,
        };
        let v3 = Vertex {
            position: [x + w, y],
            uv: [uv_max[0], uv_min[1]],
            color,
            mode,
        };

        self.vertices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
    }

    /// Upload vertices to the GPU.
    pub fn prepare(&mut self, ctx: &WgpuContext, width: u32, height: u32) {
        if self.vertices.is_empty() {
            return;
        }

        let new_size = [width as f32, height as f32];
        // Sizes originate from integer pixel counts, so exact comparison is intentional
        // and never sees NaN; we only want to skip the upload when nothing changed.
        #[allow(
            clippy::float_cmp,
            reason = "screen size derived from integer pixel counts"
        )]
        let size_changed = self.screen_size != new_size;
        if size_changed {
            self.screen_size = new_size;
            let data = [width as f32, height as f32, 0.0, 0.0];
            ctx.queue
                .write_buffer(&self.screen_buffer, 0, bytemuck::cast_slice(&data));
        }

        let needed_size = (self.vertices.len() * size_of::<Vertex>()) as u64;
        if needed_size > self.vertex_buffer.size() {
            self.vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GUI Vertex Buffer"),
                size: needed_size * 2,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        ctx.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
    }

    /// Clear the draw list (vertices, batches and any unbalanced clips).
    pub fn finish(&mut self) {
        self.vertices.clear();
        self.batches.clear();
        self.clip_stack.clear();
    }

    /// Render the primitives, applying each batch's scissor rectangle.
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.vertices.is_empty() {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(
            0,
            self.vertex_buffer
                .slice(0..((self.vertices.len() * size_of::<Vertex>()) as u64)),
        );

        let total = self.vertices.len() as u32;
        let sw = self.screen_size[0] as u32;
        let sh = self.screen_size[1] as u32;

        for (i, batch) in self.batches.iter().enumerate() {
            let end = self.batches.get(i + 1).map_or(total, |next| next.start);
            if end <= batch.start {
                continue;
            }
            match batch.scissor {
                Some([x, y, w, h]) => {
                    // Clamp to the surface; wgpu rejects scissors past the edge or with
                    // zero area, so fully-offscreen / empty clips skip the draw entirely.
                    let x = x.min(sw);
                    let y = y.min(sh);
                    let w = w.min(sw.saturating_sub(x));
                    let h = h.min(sh.saturating_sub(y));
                    if w == 0 || h == 0 {
                        continue;
                    }
                    pass.set_scissor_rect(x, y, w, h);
                }
                None => pass.set_scissor_rect(0, 0, sw, sh),
            }
            pass.draw(batch.start..end, 0..1);
        }
    }
}
