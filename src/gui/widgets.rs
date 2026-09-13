//! Leaf widgets on [`Ui`].

use glam::Vec2;

use crate::window::event::Key;

use super::builder::Ui;
use super::frame::LAYER_POPUP;
use super::style::{over, with_alpha, Color};
use super::types::{Id, Rect, Response, Sense};

/// Button emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    /// Plain.
    #[default]
    Normal,
    /// Accent-filled (the primary action).
    Accent,
    /// Danger-filled (stop / destructive).
    Danger,
    /// Borderless (toolbar / menu items).
    Flat,
}

/// Result of a text field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextResponse {
    /// Interaction on the field.
    pub response: Response,
    /// The text changed this frame.
    pub changed: bool,
    /// Enter was pressed while focused.
    pub submitted: bool,
}

/// Inclusive numeric range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    /// Minimum.
    pub min: f32,
    /// Maximum.
    pub max: f32,
}

impl Range {
    /// `min..=max`.
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    /// Clamp `v` into the range.
    pub fn clamp(&self, v: f32) -> f32 {
        v.clamp(self.min.min(self.max), self.max.max(self.min))
    }

    /// Unbounded.
    pub const UNBOUNDED: Self = Self {
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
    };
}

impl Ui<'_> {
    // ----- text --------------------------------------------------------------------

    /// A line of text.
    pub fn label(&mut self, text: &str) -> Response {
        let color = self.style().palette.text;
        self.label_colored(text, color)
    }

    /// A line of text in `color`.
    pub fn label_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size;
        self.text_widget(text, size, color)
    }

    /// Secondary small text.
    pub fn small(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size_small, self.style().palette.text_dim);
        self.text_widget(text, size, color)
    }

    /// Heading text.
    pub fn heading(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size_heading, self.style().palette.text);
        self.text_widget(text, size, color)
    }

    fn text_widget(&mut self, text: &str, size: f32, color: Color) -> Response {
        let measured = self.gui().measure(text, size);
        let natural = Vec2::new(measured.x, measured.y.max(size * 1.2));
        let alloc = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(alloc, Sense::HOVER, id);
        let y = r.rect.min.y + (r.rect.height() - measured.y) * 0.5;
        self.gui()
            .paint_text(text, Vec2::new(r.rect.min.x, y), size, color);
        r
    }

    /// Text wrapped to the available width (splits on whitespace).
    pub fn wrapped(&mut self, text: &str) -> Response {
        let size = self.style().font_size;
        let color = self.style().palette.text;
        let width = self.available_width();
        let mut lines: Vec<String> = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if !line.is_empty() && self.gui().measure(&candidate, size).x > width {
                    lines.push(core::mem::take(&mut line));
                    word.clone_into(&mut line);
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        let line_h = size * 1.2;
        let rect = self.allocate(Vec2::new(width, line_h * lines.len() as f32));
        for (i, l) in lines.iter().enumerate() {
            let pos = Vec2::new(rect.min.x, rect.min.y + i as f32 * line_h);
            self.gui().paint_text(l, pos, size, color);
        }
        self.interact(self.make_id(text), rect, Sense::HOVER)
    }

    // ----- buttons -----------------------------------------------------------------

    /// A button. Natural width fits the text; `set_next_width` overrides.
    pub fn button(&mut self, text: &str) -> Response {
        self.button_kind(text, ButtonKind::Normal)
    }

    /// A button with emphasis.
    pub fn button_kind(&mut self, text: &str, kind: ButtonKind) -> Response {
        let style = self.style().clone();
        let measured = self.gui().measure(text, style.font_size);
        let natural = Vec2::new(measured.x + style.padding * 2.0, style.row_height);
        let size = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let (bg, fg) = match kind {
            ButtonKind::Normal => (p.panel_alt, p.text),
            ButtonKind::Accent => (p.accent, p.text_on_accent),
            ButtonKind::Danger => (p.danger, p.text),
            ButtonKind::Flat => ([0.0; 4], p.text),
        };
        let enabled = self.enabled();
        let bg = if !enabled {
            with_alpha(bg, bg[3] * 0.5)
        } else if r.dragged || (r.hovered && self.gui().mouse_down(crate::MouseButton::Left)) {
            over(p.active, bg)
        } else if r.hovered {
            over(p.hover, bg)
        } else {
            bg
        };
        let fg = if enabled { fg } else { p.text_dim };
        self.gui().paint_rect(r.rect, bg);
        if kind == ButtonKind::Normal {
            self.gui().paint_rect_outline(r.rect, 1.0, p.line);
        }
        let pos = r.rect.min + (r.rect.size() - measured) * 0.5;
        self.gui().paint_text(text, pos, style.font_size, fg);
        r
    }

    /// A toggle-style button that shows `selected` (segment strips, tabs, tool modes).
    pub fn selectable(&mut self, text: &str, selected: bool) -> Response {
        let style = self.style().clone();
        let measured = self.gui().measure(text, style.font_size);
        let natural = Vec2::new(measured.x + style.padding * 2.0, style.row_height);
        let size = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let bg = if selected {
            p.accent
        } else if r.hovered {
            over(p.hover, p.panel_alt)
        } else {
            p.panel_alt
        };
        let fg = if selected { p.text_on_accent } else { p.text };
        self.gui().paint_rect(r.rect, bg);
        let pos = r.rect.min + (r.rect.size() - measured) * 0.5;
        self.gui().paint_text(text, pos, style.font_size, fg);
        r
    }

    /// An exclusive choice strip. Returns `true` when `current` changed.
    pub fn segment(&mut self, current: &mut usize, options: &[&str]) -> bool {
        let mut changed = false;
        let gap = self.gap();
        self.horizontal(|ui| {
            ui.set_gap(1.0);
            for (i, o) in options.iter().enumerate() {
                if ui.selectable(o, *current == i).clicked && *current != i {
                    *current = i;
                    changed = true;
                }
            }
            ui.set_gap(gap);
        });
        changed
    }

    // ----- toggles -----------------------------------------------------------------

    /// A checkbox. Returns the response; `value` is toggled on click.
    pub fn checkbox(&mut self, value: &mut bool, text: &str) -> Response {
        let style = self.style().clone();
        let box_size = style.font_size + 4.0;
        let measured = self.gui().measure(text, style.font_size);
        let natural = Vec2::new(
            box_size + style.padding + measured.x,
            style.row_height.max(box_size),
        );
        let size = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(size, Sense::CLICK, id);
        if r.clicked {
            *value = !*value;
        }
        let p = &style.palette;
        let bx = Rect::from_min_size(
            Vec2::new(r.rect.min.x, r.rect.center().y - box_size * 0.5),
            Vec2::splat(box_size),
        );
        let bg = if r.hovered {
            over(p.hover, p.panel_alt)
        } else {
            p.panel_alt
        };
        self.gui().paint_rect(bx, bg);
        self.gui()
            .paint_rect_outline(bx, 1.0, if *value { p.accent } else { p.line });
        if *value {
            self.gui().paint_rect(bx.shrink(4.0), p.accent);
        }
        let pos = Vec2::new(
            bx.max.x + style.padding,
            r.rect.center().y - measured.y * 0.5,
        );
        let fg = if self.enabled() { p.text } else { p.text_dim };
        self.gui().paint_text(text, pos, style.font_size, fg);
        r
    }

    /// A radio button; `clicked` means "select me".
    pub fn radio(&mut self, selected: bool, text: &str) -> Response {
        let style = self.style().clone();
        let d = style.font_size + 4.0;
        let measured = self.gui().measure(text, style.font_size);
        let natural = Vec2::new(d + style.padding + measured.x, style.row_height.max(d));
        let size = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let c = Vec2::new(r.rect.min.x + d * 0.5, r.rect.center().y);
        self.gui().paint_circle(
            c,
            d * 0.5,
            if r.hovered {
                over(p.hover, p.panel_alt)
            } else {
                p.panel_alt
            },
        );
        if selected {
            self.gui().paint_circle(c, d * 0.25, p.accent);
        }
        let pos = Vec2::new(r.rect.min.x + d + style.padding, c.y - measured.y * 0.5);
        self.gui().paint_text(text, pos, style.font_size, p.text);
        r
    }

    // ----- numbers -----------------------------------------------------------------

    /// A horizontal slider over `range`. Fills the available width by default.
    pub fn slider(&mut self, value: &mut f32, range: Range) -> Response {
        let style = self.style().clone();
        let natural = Vec2::new(self.available_width().max(40.0), style.row_height);
        let size = self.resolve_size(natural);
        let id = self.make_id(("slider", self.cursor().to_array().map(f32::to_bits)));
        let r = self.allocate_response(size, Sense::DRAG, id);
        let radius = style.row_height * 0.3;
        let track = Rect::from_min_size(
            Vec2::new(r.rect.min.x + radius, r.rect.center().y - 2.0),
            Vec2::new((r.rect.width() - radius * 2.0).max(1.0), 4.0),
        );
        if r.dragged || r.pressed {
            let f = ((self.gui().mouse().x - track.min.x) / track.width()).clamp(0.0, 1.0);
            *value = range.min + f * (range.max - range.min);
        }
        *value = range.clamp(*value);
        let span = range.max - range.min;
        let frac = if span.abs() > 0.0 {
            ((*value - range.min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let p = &style.palette;
        self.gui().paint_rect(track, p.line);
        let (filled, _) = track.split_left(track.width() * frac);
        self.gui().paint_rect(filled, p.accent);
        let knob = Vec2::new(track.min.x + track.width() * frac, track.center().y);
        self.gui().paint_circle(
            knob,
            radius,
            if r.dragged || r.hovered {
                p.text
            } else {
                p.text_dim
            },
        );
        r
    }

    /// A number that changes by dragging horizontally (`speed` units per pixel) and can
    /// be typed into after a double click. Shown with `decimals` digits.
    pub fn drag_value(
        &mut self,
        value: &mut f32,
        speed: f32,
        range: Range,
        decimals: usize,
    ) -> Response {
        let style = self.style().clone();
        let id = self.make_id(("drag", self.cursor().to_array().map(f32::to_bits)));
        let edit_id = id.with("edit");
        let editing = self.gui().string(edit_id).is_some();
        if editing {
            let mut buf = self.gui().string(edit_id).cloned().unwrap_or_default();
            let natural = Vec2::new(80.0, style.row_height);
            let size = self.resolve_size(natural);
            let t = self.text_input_sized(edit_id, &mut buf, size);
            let focused = self.gui().has_focus(edit_id);
            if t.submitted || !focused {
                if let Ok(v) = buf.trim().parse::<f32>() {
                    *value = range.clamp(v);
                }
                self.gui().set_string(edit_id, None);
            } else {
                self.gui().set_string(edit_id, Some(buf));
            }
            return t.response;
        }
        let text = format!("{value:.decimals$}");
        let measured = self.gui().measure(&text, style.font_size);
        let natural = Vec2::new(
            (measured.x + style.padding * 2.0).max(56.0),
            style.row_height,
        );
        let size = self.resolve_size(natural);
        let r = self.allocate_response(size, Sense::DRAG, id);
        if r.dragged && r.drag_delta.x != 0.0 {
            *value = range.clamp(*value + r.drag_delta.x * speed);
        }
        if r.double_clicked {
            self.gui().set_string(edit_id, Some(text.clone()));
            self.gui().request_focus(edit_id);
        }
        let p = &style.palette;
        let bg = if r.dragged {
            over(p.active, p.panel_alt)
        } else if r.hovered {
            over(p.hover, p.panel_alt)
        } else {
            p.panel_alt
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, p.line);
        let pos = r.rect.min + (r.rect.size() - measured) * 0.5;
        self.gui().paint_text(&text, pos, style.font_size, p.text);
        r
    }

    /// A progress bar (`0..=1`).
    pub fn progress(&mut self, fraction: f32) -> Response {
        let style = self.style().clone();
        let natural = Vec2::new(self.available_width().max(20.0), style.font_size * 0.6);
        let size = self.resolve_size(natural);
        let id = self.make_id(("progress", self.cursor().to_array().map(f32::to_bits)));
        let r = self.allocate_response(size, Sense::HOVER, id);
        self.gui().paint_rect(r.rect, style.palette.panel_alt);
        let (filled, _) = r.rect.split_left(r.rect.width() * fraction.clamp(0.0, 1.0));
        self.gui().paint_rect(filled, style.palette.accent);
        r
    }

    // ----- text input --------------------------------------------------------------

    /// A single-line text field over the available width.
    pub fn text_input(&mut self, text: &mut String) -> TextResponse {
        let h = self.style().row_height;
        let natural = Vec2::new(self.available_width().max(40.0), h);
        let size = self.resolve_size(natural);
        let id = self.make_id(("text", self.cursor().to_array().map(f32::to_bits)));
        self.text_input_sized(id, text, size)
    }

    /// A text field with an explicit id and size.
    pub fn text_input_sized(&mut self, id: Id, text: &mut String, size: Vec2) -> TextResponse {
        let style = self.style().clone();
        let r = self.allocate_response(size, Sense::CLICK, id);
        let chars: Vec<char> = text.chars().collect();
        let mut cursor = self
            .gui()
            .cursor(id)
            .unwrap_or(chars.len())
            .min(chars.len());
        let pad = style.padding;
        let text_x = r.rect.min.x + pad;

        if r.pressed {
            self.gui().set_focus(Some(id));
            // Place the caret at the nearest character boundary.
            let mx = self.gui().mouse().x;
            let mut best = chars.len();
            let mut best_d = f32::INFINITY;
            for i in 0..=chars.len() {
                let prefix: String = chars[..i].iter().collect();
                let w = self.gui().measure(&prefix, style.font_size).x;
                let d = (text_x + w - mx).abs();
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            cursor = best;
        }
        let focused = self.gui().has_focus(id);
        let mut changed = false;
        let mut submitted = false;
        let mut chars = chars;
        if focused && self.enabled() {
            self.gui().use_keyboard();
            let typed = self.gui().typed().to_owned();
            if !typed.is_empty() {
                for c in typed.chars() {
                    chars.insert(cursor, c);
                    cursor += 1;
                }
                changed = true;
            }
            let keys: Vec<Key> = self.gui().keys_pressed().to_vec();
            for k in keys {
                match k {
                    Key::Backspace if cursor > 0 => {
                        let _ = chars.remove(cursor - 1);
                        cursor -= 1;
                        changed = true;
                    }
                    Key::Delete if cursor < chars.len() => {
                        let _ = chars.remove(cursor);
                        changed = true;
                    }
                    Key::Left => cursor = cursor.saturating_sub(1),
                    Key::Right => cursor = (cursor + 1).min(chars.len()),
                    Key::Home => cursor = 0,
                    Key::End => cursor = chars.len(),
                    Key::Enter => submitted = true,
                    Key::Escape => self.gui().set_focus(None),
                    _ => {}
                }
            }
            if changed {
                *text = chars.iter().collect();
            }
        }
        self.gui().set_cursor(id, cursor);

        let p = &style.palette;
        let bg = if focused {
            p.panel_alt
        } else {
            over(with_alpha(p.panel_alt, 0.7), p.panel)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui()
            .paint_rect_outline(r.rect, 1.0, if focused { p.accent } else { p.line });
        let inner = r.rect.shrink2(Vec2::new(pad, 0.0));
        let measured = self.gui().measure(text, style.font_size);
        let y = r.rect.center().y - measured.y * 0.5;
        let fg = if self.enabled() { p.text } else { p.text_dim };
        self.gui().push_clip(inner);
        self.gui()
            .paint_text(text, Vec2::new(text_x, y), style.font_size, fg);
        if focused && (self.gui().time() * 2.0).fract() < 0.5 {
            let prefix: String = chars[..cursor.min(chars.len())].iter().collect();
            let cx = text_x + self.gui().measure(&prefix, style.font_size).x;
            let caret = Rect::new(cx, y, 1.5, measured.y.max(style.font_size * 1.2));
            self.gui().paint_rect(caret, p.text);
        }
        self.gui().pop_clip();
        TextResponse {
            response: r,
            changed,
            submitted,
        }
    }

    // ----- choice --------------------------------------------------------------------

    /// A drop-down over `options`. Returns `true` when `current` changed.
    pub fn combo(&mut self, current: &mut usize, options: &[&str]) -> bool {
        let style = self.style().clone();
        let id = self.make_id(("combo", self.cursor().to_array().map(f32::to_bits)));
        let shown = options.get(*current).copied().unwrap_or("");
        let widest = options
            .iter()
            .map(|o| self.gui().measure(o, style.font_size).x)
            .fold(0.0_f32, f32::max);
        let natural = Vec2::new(
            widest + style.padding * 3.0 + style.font_size,
            style.row_height,
        );
        let size = self.resolve_size(natural);
        let r = self.allocate_response(size, Sense::CLICK, id);
        if r.clicked {
            let was = self.gui().was_open(id);
            self.gui().set_open(id, !was);
        }
        let p = &style.palette;
        let bg = if r.hovered {
            over(p.hover, p.panel_alt)
        } else {
            p.panel_alt
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, p.line);
        let measured = self.gui().measure(shown, style.font_size);
        let pos = Vec2::new(
            r.rect.min.x + style.padding,
            r.rect.center().y - measured.y * 0.5,
        );
        self.gui().paint_text(shown, pos, style.font_size, p.text);
        // Chevron.
        let c = Vec2::new(r.rect.max.x - style.padding - 5.0, r.rect.center().y);
        self.gui().paint_line(
            c + Vec2::new(-4.0, -2.0),
            c + Vec2::new(0.0, 2.0),
            1.5,
            p.text_dim,
        );
        self.gui().paint_line(
            c + Vec2::new(0.0, 2.0),
            c + Vec2::new(4.0, -2.0),
            1.5,
            p.text_dim,
        );

        let mut changed = false;
        if self.gui().is_open(id) {
            let width = r.rect.width();
            let row = style.row_height;
            let height = row * options.len() as f32 + 2.0;
            self.popup(id, r.rect, Vec2::new(width, height), |ui| {
                ui.set_gap(0.0);
                for (i, o) in options.iter().enumerate() {
                    ui.set_next_width(width - 2.0);
                    if ui.selectable(o, i == *current).clicked {
                        if *current != i {
                            *current = i;
                            changed = true;
                        }
                        ui.gui().set_open(id, false);
                    }
                }
            });
        }
        changed
    }

    /// Draw `add` in a popup below `anchor` while `id` is open. The popup lives on the
    /// popup layer and blocks the base layer until closed.
    pub fn popup(&mut self, id: Id, anchor: Rect, size: Vec2, add: impl FnOnce(&mut Ui<'_>)) {
        if !self.gui().is_open(id) {
            return;
        }
        let style = self.style().clone();
        let win = self.gui().size();
        let mut pos = Vec2::new(anchor.min.x, anchor.max.y + 1.0);
        if pos.y + size.y > win.y {
            pos.y = (anchor.min.y - size.y - 1.0).max(0.0);
        }
        if pos.x + size.x > win.x {
            pos.x = (win.x - size.x).max(0.0);
        }
        let rect = Rect::from_min_size(pos, size);
        let prev_layer = self.gui().layer();
        self.gui().set_layer(LAYER_POPUP);
        self.gui().register_popup(rect);
        self.gui().paint_rect(rect, style.palette.popup);
        self.gui().paint_rect_outline(rect, 1.0, style.palette.line);
        let inner = rect.shrink(1.0);
        let enabled = self.enabled();
        {
            let mut c = self.child(id.with("popup"), inner, super::types::Dir::Vertical);
            c.set_enabled(enabled);
            c.gui().push_clip(inner);
            add(&mut c);
            c.gui().pop_clip();
        }
        self.gui().set_layer(prev_layer);
    }

    // ----- misc ----------------------------------------------------------------------

    /// A thin separator line across the layout direction.
    pub fn separator(&mut self) {
        let style = self.style().clone();
        let rect = match self.dir() {
            super::types::Dir::Vertical => self.allocate(Vec2::new(self.available_width(), 1.0)),
            super::types::Dir::Horizontal => self.allocate(Vec2::new(1.0, style.row_height)),
        };
        self.gui().paint_rect(rect, style.palette.line);
    }

    /// Show `text` as a tooltip when `response`'s widget is hovered long enough.
    pub fn tooltip(&mut self, response: &Response, text: &str) {
        self.gui().tooltip(response.id, text);
    }
}
