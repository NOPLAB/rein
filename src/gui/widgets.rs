//! Leaf widgets on [`Ui`].
//!
//! The look follows an instrument-panel convention: 20 px text rows, 22 px controls,
//! square corners, no bevels. Depth is surface brightness plus a 1 px line; the only hues
//! are the accent (selection / ON), danger and ok.

use glam::Vec2;

use crate::window::event::{Cursor, Key};

use super::builder::Ui;
use super::frame::LAYER_POPUP;
use super::style::{over, with_alpha, Color};
use super::text::TextStyle;
use super::types::{Dir, Id, Rect, Response, Sense};

/// Button emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    /// Raised surface with a border.
    #[default]
    Normal,
    /// Accent-filled (the primary action).
    Accent,
    /// Danger outline (red text and border on the raised surface).
    Danger,
    /// Danger-filled (a dangerous state is *on*: E-STOP engaged).
    DangerFilled,
    /// Borderless (toolbar / menu / row buttons); a border appears on hover.
    Flat,
}

/// Explicit button colours for looks the [`ButtonKind`]s do not cover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonStyle {
    /// Surface.
    pub bg: Color,
    /// Surface while hovered.
    pub bg_hover: Color,
    /// Surface while pressed.
    pub bg_active: Color,
    /// Text.
    pub fg: Color,
    /// Text while pressed (`None` = `fg`).
    pub fg_active: Option<Color>,
    /// 1 px border (`None` = none).
    pub border: Option<Color>,
    /// Border while hovered (`None` = `border`).
    pub border_hover: Option<Color>,
    /// Font size (`None` = the style's base size).
    pub font_size: Option<f32>,
    /// Text attributes.
    pub text: TextStyle,
}

impl ButtonStyle {
    /// The colours for `kind` from the current palette.
    pub fn for_kind(ui: &Ui<'_>, kind: ButtonKind) -> Self {
        let p = &ui.style().palette;
        let base = Self {
            bg: p.raised,
            bg_hover: p.raised_hover,
            bg_active: p.raised_active,
            fg: p.text,
            fg_active: None,
            border: Some(p.line),
            border_hover: None,
            font_size: None,
            text: TextStyle::NORMAL,
        };
        match kind {
            ButtonKind::Normal => base,
            ButtonKind::Accent => Self {
                bg: p.accent,
                bg_hover: over([1.0, 1.0, 1.0, 0.08], p.accent),
                bg_active: over([0.0, 0.0, 0.0, 0.1], p.accent),
                fg: p.text_on_accent,
                border: None,
                ..base
            },
            ButtonKind::Danger => Self {
                fg: p.danger,
                border: Some(p.danger),
                ..base
            },
            ButtonKind::DangerFilled => Self {
                bg: p.danger,
                bg_hover: over([0.0, 0.0, 0.0, 0.12], p.danger),
                bg_active: over([0.0, 0.0, 0.0, 0.2], p.danger),
                fg: [1.0, 1.0, 1.0, 1.0],
                border: None,
                ..base
            },
            ButtonKind::Flat => Self {
                bg: [0.0; 4],
                bg_hover: p.raised,
                bg_active: p.raised_active,
                border: None,
                border_hover: Some(p.line),
                ..base
            },
        }
    }
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

    fn fraction(self, v: f32) -> f32 {
        let span = self.max - self.min;
        if span.abs() > 0.0 {
            ((v - self.min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl Ui<'_> {
    // ----- text --------------------------------------------------------------------

    /// A line of primary text.
    pub fn label(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size, self.style().palette.text);
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// A line of text in `color`.
    pub fn label_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size;
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// Secondary text: a row label (`.label`), base size in the dim colour.
    pub fn dim(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size, self.style().palette.text_dim);
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// Tertiary small text (hints, captions).
    pub fn small(&mut self, text: &str) -> Response {
        let (size, color) = (
            self.style().font_size_small,
            self.style().palette.text_faint,
        );
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// Small text in `color`.
    pub fn small_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size_small;
        self.text_widget(text, size, color, TextStyle::NORMAL)
    }

    /// A monospace value (`.value`): small, tabular, primary colour.
    pub fn mono(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size_small, self.style().palette.text);
        self.text_widget(text, size, color, TextStyle::MONO)
    }

    /// A monospace value in `color`.
    pub fn mono_colored(&mut self, text: &str, color: Color) -> Response {
        let size = self.style().font_size_small;
        self.text_widget(text, size, color, TextStyle::MONO)
    }

    /// A section heading: a full-width row with small-caps style text and a rule under
    /// it. In a horizontal layout it takes only its natural width.
    pub fn heading(&mut self, text: &str) -> Response {
        let style = self.style().clone();
        let ts = TextStyle::SEMIBOLD.spaced(0.06);
        let measured = self.gui().measure_styled(text, style.font_size_heading, ts);
        let natural = match self.dir() {
            Dir::Vertical => Vec2::new(self.available_width(), style.row_height),
            Dir::Horizontal => Vec2::new(measured.x + 2.0, style.row_height),
        };
        let size = self.resolve_size(natural);
        let id = self.make_id(("heading", text));
        let r = self.allocate_response(size, Sense::HOVER, id);
        let p = &style.palette;
        let rule = Rect::new(r.rect.min.x, r.rect.max.y - 1.0, r.rect.width(), 1.0);
        self.gui().paint_rect(rule, p.line);
        let y = r.rect.min.y + (r.rect.height() - 1.0 - measured.y) * 0.5;
        self.gui().paint_text_styled(
            text,
            Vec2::new(r.rect.min.x, y),
            style.font_size_heading,
            p.text_faint,
            ts,
        );
        // Room under the rule so the first row does not touch it.
        self.add_space(3.0);
        r
    }

    /// Text in a custom size / colour / attributes.
    pub fn text_styled(&mut self, text: &str, size: f32, color: Color, ts: TextStyle) -> Response {
        self.text_widget(text, size, color, ts)
    }

    fn text_widget(&mut self, text: &str, size: f32, color: Color, ts: TextStyle) -> Response {
        let row = self.style().row_height;
        let measured = self.gui().measure_styled(text, size, ts);
        let natural = Vec2::new(
            measured.x,
            row.min(measured.y.max(size * 1.2)).max(measured.y),
        );
        let alloc = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(alloc, Sense::HOVER, id);
        let y = r.rect.min.y + (r.rect.height() - measured.y) * 0.5;
        let color = if self.enabled() {
            color
        } else {
            self.style().palette.text_disabled
        };
        self.gui()
            .paint_text_styled(text, Vec2::new(r.rect.min.x, y), size, color, ts);
        r
    }

    /// Text wrapped to the available width (splits on whitespace, and between CJK
    /// characters).
    pub fn wrapped(&mut self, text: &str) -> Response {
        let (size, color) = (self.style().font_size, self.style().palette.text);
        self.wrapped_styled(text, size, color, 1.4)
    }

    /// A wrapped hint (`.hint`): small, faint, generous line height.
    pub fn hint(&mut self, text: &str) -> Response {
        let (size, color) = (
            self.style().font_size_small,
            self.style().palette.text_faint,
        );
        self.wrapped_styled(text, size, color, 1.4)
    }

    /// Wrapped text in a custom size / colour.
    pub fn wrapped_styled(
        &mut self,
        text: &str,
        size: f32,
        color: Color,
        leading: f32,
    ) -> Response {
        let width = self.available_width().max(1.0);
        let mut lines: Vec<String> = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in wrap_units(paragraph) {
                let candidate = format!("{line}{word}");
                if !line.is_empty() && self.gui().measure(candidate.trim_end(), size).x > width {
                    lines.push(line.trim_end().to_owned());
                    word.trim_start().clone_into(&mut line);
                } else {
                    line = candidate;
                }
            }
            lines.push(line.trim_end().to_owned());
        }
        let line_h = size * leading;
        let rect = self.allocate(Vec2::new(width, line_h * lines.len() as f32));
        let color = if self.enabled() {
            color
        } else {
            self.style().palette.text_disabled
        };
        for (i, l) in lines.iter().enumerate() {
            let pos = Vec2::new(
                rect.min.x,
                rect.min.y + i as f32 * line_h + (line_h - size * 1.2) * 0.5,
            );
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
        let bs = ButtonStyle::for_kind(self, kind);
        self.button_styled(text, bs)
    }

    /// A button with explicit colours.
    pub fn button_styled(&mut self, text: &str, bs: ButtonStyle) -> Response {
        let style = self.style().clone();
        let font = bs.font_size.unwrap_or(style.font_size);
        let measured = self.gui().measure_styled(text, font, bs.text);
        let natural = Vec2::new(
            measured.x + style.button_padding * 2.0,
            style.control_height,
        );
        let size = self.resolve_size(natural);
        let id = self.make_id(text);
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let enabled = self.enabled();
        let pressed = self.gui().is_active(id) && self.gui().mouse_down(crate::MouseButton::Left);
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, Some(p.line))
        } else if pressed {
            (bs.bg_active, bs.fg_active.unwrap_or(bs.fg), bs.border)
        } else if r.hovered {
            (bs.bg_hover, bs.fg, bs.border_hover.or(bs.border))
        } else {
            (bs.bg, bs.fg, bs.border)
        };
        self.gui().paint_rect(r.rect, bg);
        if let Some(b) = border {
            self.gui().paint_rect_outline(r.rect, 1.0, b);
        }
        let pos = r.rect.min + (r.rect.size() - measured) * 0.5;
        self.gui().paint_text_styled(text, pos, font, fg, bs.text);
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    /// An icon button: a square control whose glyph is painted by `paint` into the
    /// given rect (colour = the text colour for the state).
    pub fn icon_button(
        &mut self,
        id_source: impl core::hash::Hash,
        selected: bool,
        paint: impl FnOnce(&mut Ui<'_>, Rect, Color),
    ) -> Response {
        let style = self.style().clone();
        let side = style.control_height;
        let size = self.resolve_size(Vec2::splat(side));
        let id = self.make_id(("icon", id_source));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let enabled = self.enabled();
        let pressed = self.gui().is_active(id) && self.gui().mouse_down(crate::MouseButton::Left);
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, p.line)
        } else if selected {
            (
                if r.hovered {
                    p.accent_bg_hover
                } else {
                    p.accent_bg
                },
                p.accent,
                p.accent,
            )
        } else if pressed {
            (p.raised_active, p.text, p.line)
        } else if r.hovered {
            (p.raised_hover, p.text, p.line)
        } else {
            (p.raised, p.text, p.line)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let inner = r.rect.shrink((side - 16.0).max(0.0) * 0.5);
        paint(self, inner, fg);
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    /// A toggle-style button that shows `selected` (`.btn.on`): the accent-tinted
    /// surface with accent text and border.
    pub fn selectable(&mut self, text: &str, selected: bool) -> Response {
        let p = self.style().palette;
        let mut bs = ButtonStyle::for_kind(self, ButtonKind::Normal);
        if selected {
            bs.bg = p.accent_bg;
            bs.bg_hover = p.accent_bg_hover;
            bs.bg_active = p.accent_bg;
            bs.fg = p.accent;
            bs.border = Some(p.accent);
        }
        self.button_styled(text, bs)
    }

    /// An exclusive choice strip (`.seg`). Returns `true` when `current` changed.
    pub fn segment(&mut self, current: &mut usize, options: &[&str]) -> bool {
        let on: Vec<bool> = (0..options.len()).map(|i| i == *current).collect();
        match self.segment_strip(options, &on) {
            Some(i) if i != *current => {
                *current = i;
                true
            }
            _ => false,
        }
    }

    /// A strip of independent toggles in one frame (log levels). Returns the index that
    /// was clicked (its flag is flipped).
    pub fn segment_toggles(&mut self, options: &[&str], on: &mut [bool]) -> Option<usize> {
        let snapshot: Vec<bool> = on.to_vec();
        let hit = self.segment_strip(options, &snapshot);
        if let Some(i) = hit {
            if let Some(v) = on.get_mut(i) {
                *v = !*v;
            }
        }
        hit
    }

    /// One outer border, 1 px dividers, ON = accent tint + 2 px accent underline.
    fn segment_strip(&mut self, options: &[&str], on: &[bool]) -> Option<usize> {
        let style = self.style().clone();
        let p = style.palette;
        let font = style.font_size;
        let widths: Vec<f32> = options
            .iter()
            .map(|o| self.gui().measure(o, font).x + style.button_padding * 2.0)
            .collect();
        let natural = Vec2::new(
            widths.iter().sum::<f32>() + (options.len() as f32 - 1.0).max(0.0) + 2.0,
            style.control_height,
        );
        let size = self.resolve_size(natural);
        let id = self.make_id(("seg", options.join("|")));
        let outer = self.allocate(size);
        let enabled = self.enabled();
        // Distribute a forced width evenly.
        let scale = if natural.x > 2.0 {
            (size.x - 2.0 - (options.len() as f32 - 1.0).max(0.0))
                / (natural.x - 2.0 - (options.len() as f32 - 1.0).max(0.0))
        } else {
            1.0
        };
        let mut hit = None;
        let mut x = outer.min.x + 1.0;
        for (i, o) in options.iter().enumerate() {
            let w = widths[i] * scale;
            let cell = Rect::new(x, outer.min.y + 1.0, w, outer.height() - 2.0);
            let r = self.interact(id.with(i), cell, Sense::CLICK);
            let pressed =
                self.gui().is_active(id.with(i)) && self.gui().mouse_down(crate::MouseButton::Left);
            let selected = on.get(i).copied().unwrap_or(false);
            let (bg, fg) = if !enabled {
                ([0.0; 4], p.text_disabled)
            } else if selected {
                (
                    if r.hovered {
                        p.accent_bg_hover
                    } else {
                        p.accent_bg
                    },
                    p.accent,
                )
            } else if pressed {
                (p.raised_active, p.text)
            } else if r.hovered {
                (p.raised_hover, p.text)
            } else {
                ([0.0; 4], p.text)
            };
            self.gui().paint_rect(cell, bg);
            if selected && enabled {
                self.gui().paint_rect(
                    Rect::new(cell.min.x, cell.max.y - 2.0, cell.width(), 2.0),
                    p.accent,
                );
            }
            let m = self.gui().measure(o, font);
            let pos = cell.min + (cell.size() - m) * 0.5;
            self.gui().paint_text(o, pos, font, fg);
            if r.hovered && enabled {
                self.gui().set_cursor(Cursor::Pointer);
            }
            if r.clicked {
                hit = Some(i);
            }
            x += w;
            if i + 1 < options.len() {
                self.gui()
                    .paint_rect(Rect::new(x, cell.min.y, 1.0, cell.height()), p.line);
                x += 1.0;
            }
        }
        self.gui().paint_rect_outline(outer, 1.0, p.line);
        hit
    }

    // ----- toggles -----------------------------------------------------------------

    /// A checkbox: an 11 px sunken box, accent-filled with a dark check when on.
    pub fn checkbox(&mut self, value: &mut bool, text: &str) -> Response {
        let style = self.style().clone();
        let box_size = (style.font_size * 0.96).round();
        let measured = self.gui().measure(text, style.font_size);
        let gap = if text.is_empty() { 0.0 } else { 4.0 };
        let natural = Vec2::new(box_size + gap + measured.x, style.row_height);
        let size = self.resolve_size(natural);
        let id = self.make_id(("check", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let enabled = self.enabled();
        if r.clicked {
            *value = !*value;
        }
        let p = &style.palette;
        let bx = Rect::from_min_size(
            Vec2::new(r.rect.min.x, (r.rect.center().y - box_size * 0.5).round()),
            Vec2::splat(box_size),
        );
        if !enabled {
            self.gui().paint_rect(bx, p.background);
            self.gui().paint_rect_outline(bx, 1.0, p.line);
        } else if *value {
            self.gui().paint_rect(bx, p.accent);
            // Check mark: two strokes.
            let c = bx.center();
            let k = box_size / 11.0;
            self.gui().paint_line(
                c + Vec2::new(-3.2 * k, 0.2 * k),
                c + Vec2::new(-k, 2.4 * k),
                1.6 * k,
                p.text_on_accent,
            );
            self.gui().paint_line(
                c + Vec2::new(-k, 2.4 * k),
                c + Vec2::new(3.4 * k, -2.4 * k),
                1.6 * k,
                p.text_on_accent,
            );
        } else {
            self.gui().paint_rect(bx, p.sunken);
            self.gui().paint_rect_outline(bx, 1.0, p.line_hard);
        }
        if !text.is_empty() {
            let pos = Vec2::new(bx.max.x + gap, r.rect.center().y - measured.y * 0.5);
            let fg = if enabled { p.text_dim } else { p.text_disabled };
            self.gui().paint_text(text, pos, style.font_size, fg);
        }
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    /// A radio button; `clicked` means "select me".
    pub fn radio(&mut self, selected: bool, text: &str) -> Response {
        let style = self.style().clone();
        let d = (style.font_size * 0.96).round();
        let measured = self.gui().measure(text, style.font_size);
        let natural = Vec2::new(d + 4.0 + measured.x, style.row_height);
        let size = self.resolve_size(natural);
        let id = self.make_id(("radio", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let p = &style.palette;
        let c = Vec2::new(r.rect.min.x + d * 0.5, r.rect.center().y);
        self.gui().paint_circle(c, d * 0.5, p.line_hard);
        self.gui().paint_circle(c, d * 0.5 - 1.0, p.sunken);
        if selected {
            self.gui().paint_circle(c, d * 0.28, p.accent);
        }
        let pos = Vec2::new(r.rect.min.x + d + 4.0, c.y - measured.y * 0.5);
        let fg = if self.enabled() {
            p.text_dim
        } else {
            p.text_disabled
        };
        self.gui().paint_text(text, pos, style.font_size, fg);
        if r.hovered && self.enabled() {
            self.gui().set_cursor(Cursor::Pointer);
        }
        r
    }

    // ----- numbers -----------------------------------------------------------------

    /// A horizontal slider over `range`. Fills the available width by default.
    pub fn slider(&mut self, value: &mut f32, range: Range) -> Response {
        self.slider_with_marker(value, range, None)
    }

    /// A slider with an optional measured-value marker (a tall thin line) on the track.
    pub fn slider_with_marker(
        &mut self,
        value: &mut f32,
        range: Range,
        marker: Option<f32>,
    ) -> Response {
        let style = self.style().clone();
        let natural = Vec2::new(self.available_width().max(40.0), style.row_height);
        let size = self.resolve_size(natural);
        let id = self.make_id(("slider", self.cursor().to_array().map(f32::to_bits)));
        let r = self.allocate_response(size, Sense::DRAG, id);
        let enabled = self.enabled();
        let radius = 5.0;
        let track = Rect::from_min_size(
            Vec2::new(r.rect.min.x + radius, r.rect.center().y - 1.5),
            Vec2::new((r.rect.width() - radius * 2.0).max(1.0), 3.0),
        );
        if enabled && (r.dragged || r.pressed) {
            let f = ((self.gui().mouse().x - track.min.x) / track.width()).clamp(0.0, 1.0);
            *value = range.min + f * (range.max - range.min);
        }
        *value = range.clamp(*value);
        let frac = range.fraction(*value);
        let p = &style.palette;
        self.gui().paint_rect(track, p.line);
        let (filled, _) = track.split_left(track.width() * frac);
        self.gui()
            .paint_rect(filled, if enabled { p.accent } else { p.text_disabled });
        let knob = Vec2::new(track.min.x + track.width() * frac, track.center().y);
        self.gui().paint_circle(knob, radius, p.panel);
        self.gui().paint_circle(
            knob,
            radius - 1.0,
            if enabled { p.text } else { p.text_disabled },
        );
        if let Some(m) = marker {
            let x = track.min.x + track.width() * range.fraction(m);
            self.gui().paint_rect(
                Rect::new(x - 1.0, track.center().y - 8.5, 2.0, 17.0),
                if enabled { p.text } else { p.text_disabled },
            );
        }
        if (r.hovered || r.dragged) && enabled {
            self.gui().set_cursor(Cursor::EwResize);
        }
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
            let natural = Vec2::new(80.0, style.control_height);
            let size = self.resolve_size(natural);
            let t = self.text_field(edit_id, &mut buf, size, "");
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
        let measured = self
            .gui()
            .measure_styled(&text, style.font_size, TextStyle::MONO);
        let natural = Vec2::new((measured.x + 10.0).max(56.0), style.control_height);
        let size = self.resolve_size(natural);
        let r = self.allocate_response(size, Sense::DRAG, id);
        let enabled = self.enabled();
        if enabled && r.dragged && r.drag_delta.x != 0.0 {
            *value = range.clamp(*value + r.drag_delta.x * speed);
        }
        if enabled && r.double_clicked {
            self.gui().set_string(edit_id, Some(text.clone()));
            self.gui().request_focus(edit_id);
        }
        let p = &style.palette;
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, p.line_hard)
        } else if r.hovered || r.dragged {
            (p.sunken, p.text, p.text_faint)
        } else {
            (p.sunken, p.text, p.line_hard)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let pos = Vec2::new(r.rect.min.x + 5.0, r.rect.center().y - measured.y * 0.5);
        self.gui()
            .paint_text_styled(&text, pos, style.font_size, fg, TextStyle::MONO);
        if (r.hovered || r.dragged) && enabled {
            self.gui().set_cursor(Cursor::EwResize);
        }
        r
    }

    /// A progress bar (`0..=1`): a sunken groove with an accent fill.
    pub fn progress(&mut self, fraction: f32) -> Response {
        let style = self.style().clone();
        let natural = Vec2::new(self.available_width().max(20.0), 5.0);
        let size = self.resolve_size(natural);
        let id = self.make_id(("progress", self.cursor().to_array().map(f32::to_bits)));
        let r = self.allocate_response(size, Sense::HOVER, id);
        self.gui().paint_rect(r.rect, style.palette.sunken);
        let (filled, _) = r.rect.split_left(r.rect.width() * fraction.clamp(0.0, 1.0));
        self.gui().paint_rect(filled, style.palette.accent);
        r
    }

    // ----- text input --------------------------------------------------------------

    /// A single-line text field over the available width.
    pub fn text_input(&mut self, text: &mut String) -> TextResponse {
        self.text_input_hint(text, "")
    }

    /// A text field with a placeholder shown while empty.
    pub fn text_input_hint(&mut self, text: &mut String, hint: &str) -> TextResponse {
        let h = self.style().control_height;
        let natural = Vec2::new(self.available_width().max(40.0), h);
        let size = self.resolve_size(natural);
        let id = self.make_id(("text", self.cursor().to_array().map(f32::to_bits)));
        self.text_field(id, text, size, hint)
    }

    /// A text field with an explicit id and size.
    pub fn text_input_sized(&mut self, id: Id, text: &mut String, size: Vec2) -> TextResponse {
        self.text_field(id, text, size, "")
    }

    fn text_field(&mut self, id: Id, text: &mut String, size: Vec2, hint: &str) -> TextResponse {
        let style = self.style().clone();
        let ts = TextStyle::MONO;
        let r = self.allocate_response(size, Sense::CLICK, id);
        let chars: Vec<char> = text.chars().collect();
        let mut cursor = self
            .gui()
            .text_cursor(id)
            .unwrap_or(chars.len())
            .min(chars.len());
        let pad = 5.0;
        let text_x = r.rect.min.x + pad;
        let enabled = self.enabled();

        if r.pressed && enabled {
            self.gui().set_focus(Some(id));
            // Place the caret at the nearest character boundary.
            let mx = self.gui().mouse().x;
            let mut best = chars.len();
            let mut best_d = f32::INFINITY;
            for i in 0..=chars.len() {
                let prefix: String = chars[..i].iter().collect();
                let w = self.gui().measure_styled(&prefix, style.font_size, ts).x;
                let d = (text_x + w - mx).abs();
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            cursor = best;
        }
        let focused = self.gui().has_focus(id) && enabled;
        let mut changed = false;
        let mut submitted = false;
        let mut chars = chars;
        if focused {
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
        self.gui().set_text_cursor(id, cursor);

        let p = &style.palette;
        let (bg, border) = if !enabled {
            (p.background, p.line_hard)
        } else if focused {
            (p.sunken, p.accent)
        } else if r.hovered {
            (p.sunken, p.text_faint)
        } else {
            (p.sunken, p.line_hard)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let inner = r.rect.shrink2(Vec2::new(pad, 0.0));
        let measured = self.gui().measure_styled(text, style.font_size, ts);
        let y = r.rect.center().y - measured.y.max(style.font_size * 1.2) * 0.5;
        let fg = if enabled { p.text } else { p.text_disabled };
        self.gui().push_clip(inner);
        if text.is_empty() && !hint.is_empty() && !focused {
            self.gui()
                .paint_text(hint, Vec2::new(text_x, y), style.font_size, p.text_faint);
        } else {
            self.gui()
                .paint_text_styled(text, Vec2::new(text_x, y), style.font_size, fg, ts);
        }
        if focused && (self.gui().time() * 2.0).fract() < 0.5 {
            let prefix: String = chars[..cursor.min(chars.len())].iter().collect();
            let cx = text_x + self.gui().measure_styled(&prefix, style.font_size, ts).x;
            let caret = Rect::new(cx, y + 1.0, 1.0, style.font_size * 1.2 - 2.0);
            self.gui().paint_rect(caret, p.text);
        }
        self.gui().pop_clip();
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Text);
        }
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
        let natural = Vec2::new(widest + 5.0 + 17.0, style.control_height);
        let size = self.resolve_size(natural);
        let r = self.allocate_response(size, Sense::CLICK, id);
        let enabled = self.enabled();
        if r.clicked && enabled {
            let was = self.gui().was_open(id);
            self.gui().set_open(id, !was);
        }
        let p = &style.palette;
        let (bg, fg, border) = if !enabled {
            (p.background, p.text_disabled, p.line_hard)
        } else if self.gui().is_open(id) {
            (p.sunken, p.text, p.accent)
        } else if r.hovered {
            (p.sunken, p.text, p.text_faint)
        } else {
            (p.sunken, p.text, p.line_hard)
        };
        self.gui().paint_rect(r.rect, bg);
        self.gui().paint_rect_outline(r.rect, 1.0, border);
        let measured = self.gui().measure(shown, style.font_size);
        let pos = Vec2::new(r.rect.min.x + 5.0, r.rect.center().y - measured.y * 0.5);
        self.gui().push_clip(r.rect.shrink2(Vec2::new(4.0, 0.0)));
        self.gui().paint_text(shown, pos, style.font_size, fg);
        self.gui().pop_clip();
        // Chevron (▾).
        let c = Vec2::new(r.rect.max.x - 9.0, r.rect.center().y + 1.0);
        let cc = if enabled { p.text_dim } else { p.text_disabled };
        self.gui()
            .paint_line(c + Vec2::new(-3.5, -2.0), c + Vec2::new(0.0, 1.5), 1.2, cc);
        self.gui()
            .paint_line(c + Vec2::new(0.0, 1.5), c + Vec2::new(3.5, -2.0), 1.2, cc);
        if r.hovered && enabled {
            self.gui().set_cursor(Cursor::Pointer);
        }

        let mut changed = false;
        if self.gui().is_open(id) {
            let width = r.rect.width().max(120.0);
            let row = style.row_height;
            let height = row * options.len() as f32 + 6.0;
            self.popup(id, r.rect, Vec2::new(width, height), |ui| {
                ui.set_gap(0.0);
                for (i, o) in options.iter().enumerate() {
                    ui.set_next_width(width - 6.0);
                    if ui.menu_row(o, i == *current, "").clicked {
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

    /// One row of a menu / drop-down: a check column, the label, an optional right-aligned
    /// hint; hover = accent tint.
    pub fn menu_row(&mut self, text: &str, checked: bool, hint: &str) -> Response {
        let style = self.style().clone();
        let p = style.palette;
        let m = self.gui().measure(text, style.font_size);
        let hint_m = if hint.is_empty() {
            Vec2::ZERO
        } else {
            self.gui()
                .measure_styled(hint, style.font_size_small, TextStyle::MONO)
        };
        let natural = Vec2::new(
            8.0 + 9.0
                + 7.0
                + m.x
                + if hint.is_empty() {
                    0.0
                } else {
                    18.0 + hint_m.x
                }
                + 8.0,
            style.row_height,
        );
        let size = self.resolve_size(natural);
        let id = self.make_id(("item", text));
        let r = self.allocate_response(size, Sense::CLICK, id);
        let enabled = self.enabled();
        if r.hovered && enabled {
            self.gui().paint_rect(r.rect, p.accent_bg);
            self.gui().set_cursor(Cursor::Pointer);
        }
        let fg = if enabled { p.text } else { p.text_disabled };
        if checked {
            let cm = self.gui().measure("✓", style.font_size);
            self.gui().paint_text(
                "✓",
                Vec2::new(r.rect.min.x + 8.0, r.rect.center().y - cm.y * 0.5),
                style.font_size,
                p.accent,
            );
        }
        self.gui().paint_text(
            text,
            Vec2::new(
                r.rect.min.x + 8.0 + 9.0 + 7.0,
                r.rect.center().y - m.y * 0.5,
            ),
            style.font_size,
            fg,
        );
        if !hint.is_empty() {
            self.gui().paint_text_styled(
                hint,
                Vec2::new(
                    r.rect.max.x - 8.0 - hint_m.x,
                    r.rect.center().y - hint_m.y * 0.5,
                ),
                style.font_size_small,
                p.text_faint,
                TextStyle::MONO,
            );
        }
        r
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
        self.gui()
            .paint_shadow(rect, Vec2::new(0.0, 6.0), 12.0, 0.35);
        self.gui().paint_rect(rect, style.palette.popup);
        self.gui()
            .paint_rect_outline(rect, 1.0, style.palette.line_hard);
        let inner = rect.shrink(3.0);
        let enabled = self.enabled();
        {
            let mut c = self.child(id.with("popup"), inner, Dir::Vertical);
            c.set_enabled(enabled);
            c.gui().push_clip(inner);
            add(&mut c);
            c.gui().pop_clip();
        }
        self.gui().set_layer(prev_layer);
    }

    // ----- misc ----------------------------------------------------------------------

    /// A thin separator: a full-width rule in a vertical layout, a short vertical rule
    /// with side margins in a horizontal one.
    pub fn separator(&mut self) {
        let style = self.style().clone();
        match self.dir() {
            Dir::Vertical => {
                let rect = self.allocate(Vec2::new(self.available_width(), 1.0));
                self.gui().paint_rect(rect, style.palette.line);
            }
            Dir::Horizontal => {
                let h = style.control_height;
                let rect = self.allocate(Vec2::new(9.0, h));
                let line = Rect::new(rect.min.x + 4.0, rect.center().y - 8.0, 1.0, 16.0);
                self.gui().paint_rect(line, style.palette.line);
            }
        }
    }

    /// A status-bar cell: small text (dim by default) with an optional 7 px light in
    /// front and a hard rule on its right. Fills the row's height.
    pub fn cell(&mut self, text: &str, color: Option<Color>, led: Option<Color>) -> Response {
        let style = self.style().clone();
        let p = style.palette;
        let font = style.font_size_small;
        let m = self.gui().measure(text, font);
        let led_w = if led.is_some() { 7.0 + 5.0 } else { 0.0 };
        let h = self.rect().height().max(style.row_height);
        let size = self.resolve_size(Vec2::new(m.x + led_w + 16.0 + 1.0, h));
        let id = self.make_id(("cell", text));
        let r = self.allocate_response(size, Sense::HOVER, id);
        let mut x = r.rect.min.x + 8.0;
        if let Some(c) = led {
            let bx = Rect::from_min_size(
                Vec2::new(x, (r.rect.center().y - 3.5).round()),
                Vec2::splat(7.0),
            );
            self.gui().paint_rect(bx, c);
            self.gui()
                .paint_rect_outline(bx, 1.0, with_alpha([0.0, 0.0, 0.0, 1.0], 0.35));
            x += led_w;
        }
        self.gui().paint_text(
            text,
            Vec2::new(x, r.rect.center().y - m.y * 0.5),
            font,
            color.unwrap_or(p.text_dim),
        );
        self.gui().paint_rect(
            Rect::new(r.rect.max.x - 1.0, r.rect.min.y, 1.0, r.rect.height()),
            p.line_hard,
        );
        r
    }

    /// A small square status light (`.led`), 7 px, in `color`.
    pub fn led(&mut self, color: Color) -> Response {
        let style = self.style().clone();
        let size = self.resolve_size(Vec2::new(7.0, style.row_height));
        let id = self.make_id(("led", self.cursor().to_array().map(f32::to_bits)));
        let r = self.allocate_response(size, Sense::HOVER, id);
        let bx = Rect::from_min_size(
            Vec2::new(r.rect.min.x, (r.rect.center().y - 3.5).round()),
            Vec2::splat(7.0),
        );
        self.gui().paint_rect(bx, color);
        self.gui()
            .paint_rect_outline(bx, 1.0, with_alpha([0.0, 0.0, 0.0, 1.0], 0.35));
        r
    }

    /// Show `text` as a tooltip when `response`'s widget is hovered long enough.
    pub fn tooltip(&mut self, response: &Response, text: &str) {
        self.gui().tooltip(response.id, text);
    }
}

/// Split a paragraph into wrap units: words with their trailing space, and single CJK
/// characters (which may break anywhere).
fn wrap_units(paragraph: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in paragraph.chars() {
        let cjk = matches!(c as u32, 0x3000..=0x30ff | 0x3400..=0x9fff | 0xf900..=0xfaff | 0xff00..=0xffef);
        if c.is_whitespace() {
            cur.push(c);
            out.push(core::mem::take(&mut cur));
        } else if cjk {
            if !cur.is_empty() {
                out.push(core::mem::take(&mut cur));
            }
            out.push(c.to_string());
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_units_split_words_and_cjk() {
        let u = wrap_units("ab cd 漢字x");
        assert_eq!(u, vec!["ab ", "cd ", "漢", "字", "x"]);
    }

    #[test]
    fn range_fraction() {
        let r = Range::new(-1.0, 1.0);
        assert!((r.fraction(0.0) - 0.5).abs() < 1e-6);
        assert_eq!(Range::new(2.0, 2.0).fraction(5.0), 0.0);
    }
}
