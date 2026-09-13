//! Line plot widget.

use glam::Vec2;

use super::builder::Ui;
use super::style::Color;
use super::types::{Rect, Response, Sense};

/// One line of a plot.
#[derive(Debug, Clone, Copy)]
pub struct Series<'a> {
    /// `(x, y)` samples in x order.
    pub points: &'a [[f32; 2]],
    /// Line colour.
    pub color: Color,
    /// Legend label (empty = none).
    pub label: &'a str,
}

/// A line plot with auto-scaled axes, grid and legend.
#[derive(Debug, Clone)]
pub struct Plot<'a> {
    series: Vec<Series<'a>>,
    height: f32,
    x_range: Option<(f32, f32)>,
    y_range: Option<(f32, f32)>,
    grid: bool,
    legend: bool,
    x_label: &'a str,
    y_label: &'a str,
}

impl Default for Plot<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Plot<'a> {
    /// An empty plot.
    pub fn new() -> Self {
        Self {
            series: Vec::new(),
            height: 140.0,
            x_range: None,
            y_range: None,
            grid: true,
            legend: true,
            x_label: "",
            y_label: "",
        }
    }

    /// Add a series.
    pub fn series(mut self, s: Series<'a>) -> Self {
        self.series.push(s);
        self
    }

    /// Add a series from points.
    pub fn line(self, points: &'a [[f32; 2]], color: Color, label: &'a str) -> Self {
        self.series(Series {
            points,
            color,
            label,
        })
    }

    /// Plot height in pixels (width fills the `Ui`).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// Fixed x range (auto when unset).
    pub fn x_range(mut self, min: f32, max: f32) -> Self {
        self.x_range = Some((min, max));
        self
    }

    /// Fixed y range (auto when unset).
    pub fn y_range(mut self, min: f32, max: f32) -> Self {
        self.y_range = Some((min, max));
        self
    }

    /// Grid lines on / off.
    pub fn grid(mut self, on: bool) -> Self {
        self.grid = on;
        self
    }

    /// Legend on / off.
    pub fn legend(mut self, on: bool) -> Self {
        self.legend = on;
        self
    }

    /// Axis labels.
    pub fn labels(mut self, x: &'a str, y: &'a str) -> Self {
        self.x_label = x;
        self.y_label = y;
        self
    }

    fn auto_range(&self, axis: usize) -> (f32, f32) {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for s in &self.series {
            for p in s.points {
                if p[axis].is_finite() {
                    lo = lo.min(p[axis]);
                    hi = hi.max(p[axis]);
                }
            }
        }
        if !lo.is_finite() || !hi.is_finite() {
            return (0.0, 1.0);
        }
        if (hi - lo).abs() < 1e-9 {
            return (lo - 1.0, hi + 1.0);
        }
        if axis == 1 {
            let m = (hi - lo) * 0.05;
            (lo - m, hi + m)
        } else {
            (lo, hi)
        }
    }

    /// Draw the plot.
    pub fn show(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style().clone();
        let width = ui.available_width().max(40.0);
        let size = ui.resolve_size(Vec2::new(width, self.height));
        let id = ui.make_id(("plot", ui.cursor().to_array().map(f32::to_bits)));
        let r = ui.allocate_response(size, Sense::HOVER, id);
        let p = &style.palette;
        ui.gui().paint_rect(r.rect, p.panel_alt);

        let (xmin, xmax) = self.x_range.unwrap_or_else(|| self.auto_range(0));
        let (ymin, ymax) = self.y_range.unwrap_or_else(|| self.auto_range(1));
        let left_w = 44.0;
        let bottom_h = style.font_size_small * 1.4 + 2.0;
        let area = Rect::from_min_max(
            r.rect.min + Vec2::new(left_w, 6.0),
            r.rect.max - Vec2::new(6.0, bottom_h),
        );
        if area.is_empty() {
            return r;
        }
        let to_px = |x: f32, y: f32| {
            Vec2::new(
                area.min.x + (x - xmin) / (xmax - xmin) * area.width(),
                area.max.y - (y - ymin) / (ymax - ymin) * area.height(),
            )
        };

        // Grid + tick labels.
        let divisions = 4;
        for i in 0..=divisions {
            let t = i as f32 / divisions as f32;
            let y = ymin + t * (ymax - ymin);
            let py = to_px(xmin, y).y;
            if self.grid {
                ui.gui().paint_line(
                    Vec2::new(area.min.x, py),
                    Vec2::new(area.max.x, py),
                    1.0,
                    p.line,
                );
            }
            let label = fmt_tick(y);
            let m = ui.gui().measure(&label, style.font_size_small);
            ui.gui().paint_text(
                &label,
                Vec2::new(area.min.x - 4.0 - m.x, py - m.y * 0.5),
                style.font_size_small,
                p.text_dim,
            );
            let x = xmin + t * (xmax - xmin);
            let px = to_px(x, ymin).x;
            if self.grid {
                ui.gui().paint_line(
                    Vec2::new(px, area.min.y),
                    Vec2::new(px, area.max.y),
                    1.0,
                    p.line,
                );
            }
            let label = fmt_tick(x);
            let m = ui.gui().measure(&label, style.font_size_small);
            let lx = (px - m.x * 0.5).clamp(area.min.x, area.max.x - m.x);
            ui.gui().paint_text(
                &label,
                Vec2::new(lx, area.max.y + 2.0),
                style.font_size_small,
                p.text_dim,
            );
        }
        ui.gui().paint_rect_outline(area, 1.0, p.line);

        // Series.
        ui.gui().push_clip(area);
        for s in &self.series {
            let pts: Vec<[f32; 2]> = s
                .points
                .iter()
                .filter(|q| q[0].is_finite() && q[1].is_finite())
                .map(|q| to_px(q[0], q[1]).to_array())
                .collect();
            if pts.len() >= 2 {
                ui.gui().paint_polyline(&pts, 1.5, s.color);
            } else if let Some(q) = pts.first() {
                ui.gui().paint_circle(Vec2::from(*q), 2.0, s.color);
            }
        }
        ui.gui().pop_clip();

        // Legend.
        if self.legend {
            let mut y = area.min.y + 4.0;
            for s in self.series.iter().filter(|s| !s.label.is_empty()) {
                let m = ui.gui().measure(s.label, style.font_size_small);
                let x = area.max.x - 6.0 - m.x;
                ui.gui().paint_rect(
                    Rect::new(x - 16.0, y, 10.0, m.y).shrink2(Vec2::new(0.0, m.y * 0.35)),
                    s.color,
                );
                ui.gui()
                    .paint_text(s.label, Vec2::new(x, y), style.font_size_small, p.text);
                y += m.y + 2.0;
            }
        }

        // Axis labels.
        if !self.x_label.is_empty() {
            let m = ui.gui().measure(self.x_label, style.font_size_small);
            ui.gui().paint_text(
                self.x_label,
                Vec2::new(area.max.x - m.x, area.max.y - m.y - 2.0),
                style.font_size_small,
                p.text_dim,
            );
        }
        if !self.y_label.is_empty() {
            ui.gui().paint_text(
                self.y_label,
                Vec2::new(area.min.x + 4.0, area.min.y + 2.0),
                style.font_size_small,
                p.text_dim,
            );
        }

        // Hover readout: nearest x on the first series.
        if r.hovered {
            let mx = ui.gui().mouse().x;
            let x = xmin + (mx - area.min.x) / area.width() * (xmax - xmin);
            let mut lines = vec![format!("x = {}", fmt_tick(x))];
            for s in &self.series {
                if let Some(q) = s
                    .points
                    .iter()
                    .min_by(|a, b| (a[0] - x).abs().total_cmp(&(b[0] - x).abs()))
                {
                    let name = if s.label.is_empty() { "y" } else { s.label };
                    lines.push(format!("{name} = {}", fmt_tick(q[1])));
                }
            }
            ui.gui().paint_line(
                Vec2::new(mx.clamp(area.min.x, area.max.x), area.min.y),
                Vec2::new(mx.clamp(area.min.x, area.max.x), area.max.y),
                1.0,
                p.text_dim,
            );
            ui.gui().tooltip(id, &lines.join("\n"));
        }
        r
    }
}

/// Compact tick label.
fn fmt_tick(v: f32) -> String {
    let a = v.abs();
    if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}
