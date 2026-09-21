//! Text rendering
//!
//! Provides text rendering using glyphon.

use crate::context::WgpuContext;
use std::collections::HashMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer, Weight,
};

/// Per-call text attributes on top of the renderer's family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Use the monospace family (see [`TextRenderer::set_mono_family`]).
    pub mono: bool,
    /// Font weight (CSS scale: 400 normal, 600 semi-bold, 700 bold).
    pub weight: u16,
    /// Extra advance per glyph, as a fraction of the font size (CSS `letter-spacing` in em).
    pub letter_spacing: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl TextStyle {
    /// Proportional, normal weight.
    pub const NORMAL: Self = Self {
        mono: false,
        weight: 400,
        letter_spacing: 0.0,
    };
    /// Monospace, normal weight.
    pub const MONO: Self = Self {
        mono: true,
        weight: 400,
        letter_spacing: 0.0,
    };
    /// Proportional, semi-bold.
    pub const SEMIBOLD: Self = Self {
        mono: false,
        weight: 600,
        letter_spacing: 0.0,
    };

    /// With `letter_spacing` (em).
    #[must_use]
    pub const fn spaced(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }
}

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
    /// Family used when [`TextStyle::mono`] is set.
    mono_family: glyphon::FamilyOwned,
    /// Layer new entries are tagged with (see [`Self::set_layer`]).
    layer: u8,
    /// `(text, size bits, style)` → `(width, height)`; shaping is the cost, and labels
    /// repeat.
    measure_cache: HashMap<(String, u32, MeasureKey), (f32, f32)>,
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
            mono_family: glyphon::FamilyOwned::Monospace,
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

    /// Pick the proportional and monospace families by name (`"Segoe UI"`, `"Consolas"`);
    /// unknown names fall back to the system defaults. Clears the measure cache.
    pub fn set_family_names(&mut self, sans: &str, mono: &str) {
        self.family = glyphon::FamilyOwned::Name(sans.into());
        self.mono_family = glyphon::FamilyOwned::Name(mono.into());
        self.measure_cache.clear();
    }

    /// Use this family for text drawn with [`TextStyle::mono`]. Clears the measure cache.
    pub fn set_mono_family(&mut self, family: glyphon::FamilyOwned) {
        self.mono_family = family;
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
    /// Queue `text` at (`x`, `y`) (top-left) in **linear** `color`.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        self.draw_text_styled(text, x, y, font_size, color, TextStyle::NORMAL);
    }

    /// [`Self::draw_text`] with explicit family / weight / letter spacing.
    pub fn draw_text_styled(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
        style: TextStyle,
    ) {
        let text = spaced(text, style.letter_spacing);
        let text = text.as_ref();
        let mut buffer = self.available_buffers.pop().unwrap_or_else(|| {
            Buffer::new(
                &mut self.font_system,
                Metrics::new(font_size, font_size * 1.2),
            )
        });

        buffer.set_metrics(Metrics::new(font_size, font_size * 1.2));
        let attrs = attrs_for(&self.family, &self.mono_family, style);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // glyphon takes sRGB and converts to linear itself (`ColorMode::Accurate`).
        let ch = |c: f32| (super::style::linear_to_srgb(c.clamp(0.0, 1.0)) * 255.0).round() as u8;
        self.entries.push(TextEntry {
            buffer,
            left: x,
            top: y,
            color: Color::rgba(
                ch(color[0]),
                ch(color[1]),
                ch(color[2]),
                (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
            ),
            clip: self.clip_stack.last().copied(),
            layer: self.layer,
        });
    }

    /// Measure text dimensions.
    pub fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        self.measure_styled(text, font_size, TextStyle::NORMAL)
    }

    /// [`Self::measure`] with explicit family / weight / letter spacing.
    pub fn measure_styled(&mut self, text: &str, font_size: f32, style: TextStyle) -> (f32, f32) {
        let key = (
            text.to_owned(),
            font_size.to_bits(),
            MeasureKey::from(style),
        );
        if let Some(&m) = self.measure_cache.get(&key) {
            return m;
        }
        let m = self.measure_uncached(text, font_size, style);
        let _ = self.measure_cache.insert(key, m);
        m
    }

    fn measure_uncached(&mut self, text: &str, font_size: f32, style: TextStyle) -> (f32, f32) {
        let text = spaced(text, style.letter_spacing);
        let text = text.as_ref();
        self.scratch_buffer
            .set_metrics(Metrics::new(font_size, font_size * 1.2));
        let attrs = attrs_for(&self.family, &self.mono_family, style);
        self.scratch_buffer
            .set_text(text, &attrs, Shaping::Advanced, None);
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

/// glyphon attributes for `style` (fields are borrowed separately so buffers and the font
/// system stay mutably available).
fn attrs_for<'a>(
    family: &'a glyphon::FamilyOwned,
    mono: &'a glyphon::FamilyOwned,
    style: TextStyle,
) -> Attrs<'a> {
    let family = if style.mono { mono } else { family };
    Attrs::new()
        .family(family.as_family())
        .weight(Weight(style.weight))
}

/// Hashable form of a [`TextStyle`] for the measure cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MeasureKey {
    mono: bool,
    weight: u16,
    spacing: u32,
}

impl From<TextStyle> for MeasureKey {
    fn from(s: TextStyle) -> Self {
        Self {
            mono: s.mono,
            weight: s.weight,
            spacing: s.letter_spacing.to_bits(),
        }
    }
}

/// Letter spacing is emulated by interleaving hair spaces (U+200A) — cosmic-text has no
/// per-glyph advance, and the headings that use it are a few characters long.
fn spaced(text: &str, letter_spacing: f32) -> std::borrow::Cow<'_, str> {
    if letter_spacing <= 0.0 || text.chars().count() < 2 {
        return std::borrow::Cow::Borrowed(text);
    }
    // One hair space is about 1/8 em; round the requested spacing to that unit.
    let n = ((letter_spacing * 8.0).round() as usize).max(1);
    let filler = "\u{200a}".repeat(n);
    let mut out = String::with_capacity(text.len() * (n + 1));
    let mut first = true;
    for c in text.chars() {
        if !first {
            out.push_str(&filler);
        }
        first = false;
        out.push(c);
    }
    std::borrow::Cow::Owned(out)
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
