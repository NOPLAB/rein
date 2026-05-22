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

/// Renderer for 2D primitives.
pub struct PrimitiveRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
    screen_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    screen_size: [f32; 2],
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
        #[allow(clippy::float_cmp, reason = "screen size derived from integer pixel counts")]
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

    /// Clear the draw list.
    pub fn finish(&mut self) {
        self.vertices.clear();
    }

    /// Render the primitives.
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
        pass.draw(0..self.vertices.len() as u32, 0..1);
    }
}
