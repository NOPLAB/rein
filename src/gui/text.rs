//! Text rendering
//!
//! Provides text rendering using glyphon.

use crate::context::WgpuContext;
use std::collections::HashMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer,
};

struct TextEntry {
    buffer: Buffer,
    left: f32,
    top: f32,
    color: Color,
    /// Clip bounds `[left, top, right, bottom]` in pixels, or `None` for unclipped.
    clip: Option<[i32; 4]>,
    /// Draw layer (see `TextRenderer::set_layer`).
    layer: u8,
}

/// Text renderer using glyphon.
pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    #[expect(
        dead_code,
        reason = "glyphon Cache kept alive for the atlas; not accessed directly"
    )]
    cache: Cache,
    atlas: TextAtlas,
    renderer: GlyphonTextRenderer,

    // Immediate mode text entries
    entries: Vec<TextEntry>,
    available_buffers: Vec<Buffer>,
    scratch_buffer: Buffer,
    clip_stack: Vec<[i32; 4]>,

    viewport: glyphon::Viewport,

    /// Font family for every draw / measure call.
    family: glyphon::FamilyOwned,
    /// Layer new entries are tagged with (see [`Self::set_layer`]).
    layer: u8,
    /// `(text, size bits)` → `(width, height)`; shaping is the cost, and labels repeat.
    measure_cache: HashMap<(String, u32), (f32, f32)>,
}

impl TextRenderer {
    /// Create a new text renderer.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&ctx.device);

        let mut atlas = TextAtlas::new(&ctx.device, &ctx.queue, &cache, format);

        let renderer = GlyphonTextRenderer::new(
            &mut atlas,
            &ctx.device,
            wgpu::MultisampleState::default(),
            None,
        );

        let scratch_buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 18.0));
        let viewport = glyphon::Viewport::new(&ctx.device, &cache);

        Self {
            font_system,
            swash_cache,
            cache,
            atlas,
            renderer,
            entries: Vec::new(),
            available_buffers: Vec::new(),
            scratch_buffer,
            clip_stack: Vec::new(),
            viewport,
            family: glyphon::FamilyOwned::Monospace,
            layer: 0,
            measure_cache: HashMap::new(),
        }
    }

    /// Use this font family for all subsequent text (`FamilyOwned::SansSerif`,
    /// `FamilyOwned::Name("Noto Sans JP".into())`, …). Clears the measure cache.
    pub fn set_family(&mut self, family: glyphon::FamilyOwned) {
        self.family = family;
        self.measure_cache.clear();
    }

    /// Register font data (TTF / OTF bytes) with the font database, so it can be selected
    /// by name with [`Self::set_family`] and used as a fallback.
    pub fn load_font_data(&mut self, data: Vec<u8>) {
        self.font_system.db_mut().load_font_data(data);
        self.measure_cache.clear();
    }

    /// Tag subsequent entries with `layer`. [`Self::render_layer`] draws one layer at a
    /// time so a caller can interleave text with other passes (popups above base UI).
    pub fn set_layer(&mut self, layer: u8) {
        self.layer = layer;
    }

    /// Highest layer index used since [`Self::begin_frame`].
    pub fn max_layer(&self) -> u8 {
        self.entries.iter().map(|e| e.layer).max().unwrap_or(0)
    }

    /// Update the viewport size.
    pub fn resize(&mut self, ctx: &WgpuContext, width: u32, height: u32) {
        self.viewport
            .update(&ctx.queue, Resolution { width, height });
    }

    /// Begin a new frame for immediate mode text rendering.
    pub fn begin_frame(&mut self) {
        while let Some(entry) = self.entries.pop() {
            self.available_buffers.push(entry.buffer);
        }
        self.clip_stack.clear();
        self.layer = 0;
        if self.measure_cache.len() > 8192 {
            self.measure_cache.clear();
        }
    }

    /// Push a clip rectangle; subsequent text is bounded to the intersection of
    /// this rectangle with any clip already on the stack. Balance with [`pop_clip`].
    ///
    /// [`pop_clip`]: Self::pop_clip
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut left = x;
        let mut top = y;
        let mut right = x + w;
        let mut bottom = y + h;
        if let Some(&[pl, pt, pr, pb]) = self.clip_stack.last() {
            left = left.max(pl as f32);
            top = top.max(pt as f32);
            right = right.min(pr as f32);
            bottom = bottom.min(pb as f32);
        }
        // Keep the rect non-inverted so glyphon never sees right < left.
        let left = left.max(0.0) as i32;
        let top = top.max(0.0) as i32;
        let right = right.max(left as f32) as i32;
        let bottom = bottom.max(top as f32) as i32;
        self.clip_stack.push([left, top, right, bottom]);
    }

    /// Pop the most recently pushed clip rectangle.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Draw text immediately (queues for render).
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        let mut buffer = self.available_buffers.pop().unwrap_or_else(|| {
            Buffer::new(
                &mut self.font_system,
                Metrics::new(font_size, font_size * 1.2),
            )
        });

        buffer.set_metrics(
            &mut self.font_system,
            Metrics::new(font_size, font_size * 1.2),
        );
        buffer.set_text(
            &mut self.font_system,
            text,
            &Attrs::new().family(self.family.as_family()),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        self.entries.push(TextEntry {
            buffer,
            left: x,
            top: y,
            color: Color::rgba(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
                (color[3] * 255.0) as u8,
            ),
            clip: self.clip_stack.last().copied(),
            layer: self.layer,
        });
    }

    /// Measure text dimensions.
    pub fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        let key = (text.to_owned(), font_size.to_bits());
        if let Some(&m) = self.measure_cache.get(&key) {
            return m;
        }
        let m = self.measure_uncached(text, font_size);
        let _ = self.measure_cache.insert(key, m);
        m
    }

    fn measure_uncached(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        self.scratch_buffer.set_metrics(
            &mut self.font_system,
            Metrics::new(font_size, font_size * 1.2),
        );
        self.scratch_buffer.set_text(
            &mut self.font_system,
            text,
            &Attrs::new().family(self.family.as_family()),
            Shaping::Advanced,
            None,
        );
        self.scratch_buffer
            .shape_until_scroll(&mut self.font_system, false);

        let mut width = 0.0_f32;
        let mut height = 0.0_f32;

        for run in self.scratch_buffer.layout_runs() {
            width = width.max(run.line_w);
            height += run.line_height;
        }

        if height == 0.0 && !text.is_empty() {
            height = font_size * 1.2;
        }

        (width, height)
    }

    /// Render every layer of text (in layer order) on top of `view`.
    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        for layer in 0..=self.max_layer() {
            self.render_layer(ctx, encoder, view, width, height, layer)?;
        }
        Ok(())
    }

    /// Render only the entries tagged with `layer`.
    pub fn render_layer(
        &mut self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        layer: u8,
    ) -> anyhow::Result<()> {
        // Update viewport
        self.viewport
            .update(&ctx.queue, Resolution { width, height });

        let mut text_areas = Vec::with_capacity(self.entries.len());

        for entry in self.entries.iter().filter(|e| e.layer == layer) {
            let bounds = entry.clip.map_or(
                TextBounds {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                },
                |[left, top, right, bottom]| TextBounds {
                    left,
                    top,
                    right,
                    bottom,
                },
            );
            text_areas.push(TextArea {
                buffer: &entry.buffer,
                left: entry.left,
                top: entry.top,
                scale: 1.0,
                bounds,
                default_color: entry.color,
                custom_glyphs: &[],
            });
        }

        self.renderer.prepare(
            &ctx.device,
            &ctx.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )?;

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.renderer
                .render(&self.atlas, &self.viewport, &mut pass)?;
        }

        Ok(())
    }

    /// Trim the atlas to free unused space.
    pub fn trim(&mut self) {
        self.atlas.trim();
    }
}

/// Helper for building text content with formatting.
pub struct TextBuilder {
    lines: Vec<String>,
}

impl TextBuilder {
    /// Create a new text builder.
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Add a line of text.
    pub fn line(mut self, text: impl Into<String>) -> Self {
        self.lines.push(text.into());
        self
    }

    /// Add an empty line.
    pub fn blank(mut self) -> Self {
        self.lines.push(String::new());
        self
    }

    /// Add a separator line.
    pub fn separator(mut self, char: char, width: usize) -> Self {
        self.lines.push(char.to_string().repeat(width));
        self
    }

    /// Build the final text string.
    pub fn build(self) -> String {
        self.lines.join("\n")
    }
}

impl Default for TextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
