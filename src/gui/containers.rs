//! Container widgets on [`Ui`]: scroll areas, collapsing headers, frames, trees, tables,
//! modals.

use glam::Vec2;

use crate::window::event::Cursor;

use super::builder::Ui;
use super::frame::{Gui, LAYER_POPUP};
use super::style::Color;
use super::text::TextStyle;
use super::types::{Dir, Id, Rect, Response, Sense};

/// Result of a tree node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeResponse {
    /// Interaction on the node's row (click selects).
    pub response: Response,
    /// Whether the node is expanded.
    pub open: bool,
}

impl Ui<'_> {
    /// A vertically scrollable region of `height` (or the remaining height). Content is
    /// laid out by `add`; the scroll offset persists per id.
    pub fn scroll_area<R>(
        &mut self,
        id_source: impl core::hash::Hash,
        height: Option<f32>,
        add: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> R {
        let style = self.style().clone();
        let id = self.make_id(("scroll", id_source));
        let h = height.unwrap_or_else(|| self.available_height());
        let area = self.allocate_row(h);
        let bar_w = style.scrollbar;
        let content_w = (area.width() - bar_w).max(0.0);
        let mut scroll = self.gui().scalar(id);
        let area_r = self.interact(id, area, Sense::HOVER);
        if area_r.hovered {
            scroll -= self.gui().scroll_delta().y;
        }
        // Clamp against last frame's content height *before* laying out, so a caller that
        // pins the offset to the end (auto-scroll) sees the end this frame, not a blank.
        // Without a previous measurement the offset is 0: a huge offset would push the
        // content so far that float precision collapses its measured height to zero.
        let prev_content_h = self.gui().scalar(id.with("content_h"));
        scroll = scroll.clamp(0.0, (prev_content_h - area.height()).max(0.0));

        let (result, content_h) = {
            let inner = Rect::new(area.min.x, area.min.y - scroll, content_w, f32::MAX / 4.0);
            let mut c = self.child(id.with("content"), inner, Dir::Vertical);
            c.set_clipped(true);
            c.gui().push_clip(area);
            let r = add(&mut c);
            c.gui().pop_clip();
            (r, c.used_size().y)
        };
        let max_scroll = (content_h - area.height()).max(0.0);
        self.gui().set_scalar(id.with("content_h"), content_h);
        // The thumb.
        if max_scroll > 0.0 {
            let track = Rect::new(area.max.x - bar_w, area.min.y, bar_w, area.height());
            let thumb_h = (area.height() * area.height() / content_h).clamp(16.0, area.height());
            let thumb_id = id.with("thumb");
            let mut thumb = Rect::new(
                track.min.x + 2.0,
                track.min.y + (scroll / max_scroll).clamp(0.0, 1.0) * (track.height() - thumb_h),
                bar_w - 4.0,
                thumb_h,
            );
            let t = self.interact(thumb_id, thumb, Sense::DRAG);
            if t.dragged {
                scroll += t.drag_delta.y * (max_scroll / (track.height() - thumb_h).max(1.0));
                thumb.min.y = track.min.y
                    + (scroll / max_scroll).clamp(0.0, 1.0) * (track.height() - thumb_h);
                thumb.max.y = thumb.min.y + thumb_h;
            }
            let c = if t.dragged || t.hovered {
                style.palette.text_faint
            } else {
                style.palette.line
            };
            self.gui().paint_rect(thumb, c);
        }
        let scroll = scroll.clamp(0.0, max_scroll);
        self.gui().set_scalar(id, scroll);
        result
    }

    /// A collapsible section with a heading row (chevron + small-caps title over a
    /// rule). Returns whether it is open.
    pub fn collapsing(
        &mut self,
        text: &str,
        default_open: bool,
        add: impl FnOnce(&mut Ui<'_>),
    ) -> bool {
        let style = self.style().clone();
        let id = self.make_id(("collapsing", text));
        let stored = self.gui().scalar(id);
        let mut open = if stored == 0.0 {
            default_open
        } else {
            stored > 0.0
        };
        let row = self.allocate_row(style.row_height);
        let r = self.interact(id, row, Sense::CLICK);
        if r.clicked {
            open = !open;
        }
        self.gui().set_scalar(id, if open { 1.0 } else { -1.0 });
        let p = style.palette;
        if r.hovered {
            self.gui().paint_rect(row, p.row_hover);
            self.gui().set_cursor(Cursor::Pointer);
        }
        let rule = Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0);
        self.gui().paint_rect(rule, p.line);
        paint_chevron(
            self.gui(),
            Vec2::new(row.min.x + 6.0, row.center().y - 0.5),
            open,
            p.text_faint,
        );
        let ts = TextStyle::SEMIBOLD.spaced(0.06);
        let measured = self.gui().measure_styled(text, style.font_size_heading, ts);
        self.gui().paint_text_styled(
            text,
            Vec2::new(row.min.x + 15.0, row.center().y - 0.5 - measured.y * 0.5),
            style.font_size_heading,
            p.text_faint,
            ts,
        );
        self.add_space(3.0);
        if open {
            add(self);
        }
        open
    }

    /// Lay out `add` shifted right by `indent`.
    pub fn indented(&mut self, indent: f32, add: impl FnOnce(&mut Ui<'_>)) {
        let avail = self.available();
        let rect = Rect::from_min_max(avail.min + Vec2::new(indent, 0.0), avail.max);
        let id = self.make_id(("indent", self.cursor().to_array().map(f32::to_bits)));
        let used = {
            let mut c = self.child(id, rect, Dir::Vertical);
            add(&mut c);
            c.used_size()
        };
        let _ = self.allocate(Vec2::new(used.x + indent, used.y));
    }

    /// A sunken panel with padding around `add`.
    pub fn frame<R>(&mut self, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let bg = self.style().palette.sunken;
        self.frame_colored(bg, add)
    }

    /// A background of `color` with padding around `add`.
    pub fn frame_colored<R>(&mut self, color: Color, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let pad = self.style().padding;
        let handle = self.gui().reserve_rect();
        let avail = self.available();
        let inner = avail.shrink(pad);
        let id = self.make_id(("frame", self.cursor().to_array().map(f32::to_bits)));
        let dir = self.dir();
        let (result, used) = {
            let mut c = self.child(id, inner, dir);
            let r = add(&mut c);
            (r, c.used_rect())
        };
        let outer = Rect::from_min_max(used.min - Vec2::splat(pad), used.max + Vec2::splat(pad));
        let outer = if dir == Dir::Vertical {
            Rect::new(avail.min.x, outer.min.y, avail.width(), outer.height())
        } else {
            outer
        };
        self.gui().fill_reserved(handle, outer, color);
        let _ = self.allocate(outer.size());
        result
    }

    /// A tree node: a caret column, a selectable label row (accent tint + 1 px accent
    /// outline when selected), children indented below when open. Open by default.
    pub fn tree_node(
        &mut self,
        id_source: impl core::hash::Hash,
        text: &str,
        selected: bool,
        leaf: bool,
        add: impl FnOnce(&mut Ui<'_>),
    ) -> TreeResponse {
        self.tree_node_with(id_source, text, selected, leaf, true, add)
    }

    /// [`Self::tree_node`] with an explicit initial state (a deep tree usually opens only
    /// its root).
    pub fn tree_node_with(
        &mut self,
        id_source: impl core::hash::Hash,
        text: &str,
        selected: bool,
        leaf: bool,
        default_open: bool,
        add: impl FnOnce(&mut Ui<'_>),
    ) -> TreeResponse {
        let style = self.style().clone();
        let id = self.make_id(("tree", id_source));
        let stored = self.gui().scalar(id);
        let mut open = !leaf
            && if stored == 0.0 {
                default_open
            } else {
                stored > 0.0
            };
        let row = self.allocate_row(style.row_height);
        let caret_w = 14.0;
        let chevron = Rect::new(row.min.x, row.min.y, caret_w, row.height());
        let label_rect = Rect::from_min_max(Vec2::new(row.min.x + caret_w, row.min.y), row.max);
        let c = self.interact(id.with("chevron"), chevron, Sense::CLICK);
        let r = self.interact(id, label_rect, Sense::CLICK);
        if !leaf && (c.clicked || r.double_clicked) {
            open = !open;
        }
        self.gui().set_scalar(id, if open { 1.0 } else { -1.0 });
        let p = style.palette;
        if selected {
            self.gui().paint_rect(row, p.accent_bg);
            self.gui().paint_rect_outline(row, 1.0, p.accent);
        } else if r.hovered || c.hovered {
            self.gui().paint_rect(row, p.row_hover);
        }
        if !leaf {
            let glyph = if open { "▾" } else { "▸" };
            let m = self.gui().measure(glyph, 9.0);
            self.gui().paint_text(
                glyph,
                Vec2::new(
                    chevron.min.x + (caret_w - m.x) * 0.5,
                    chevron.center().y - m.y * 0.5,
                ),
                9.0,
                p.text_faint,
            );
            if c.hovered {
                self.gui().set_cursor(Cursor::Pointer);
            }
        }
        let measured = self.gui().measure(text, style.font_size);
        let fg = if self.enabled() {
            p.text
        } else {
            p.text_disabled
        };
        self.gui().push_clip(label_rect);
        self.gui().paint_text(
            text,
            Vec2::new(label_rect.min.x, row.center().y - measured.y * 0.5),
            style.font_size,
            fg,
        );
        self.gui().pop_clip();
        if open {
            self.indented(style.indent, add);
        }
        TreeResponse { response: r, open }
    }

    /// A table: `weights` are relative column widths, `header` the column titles,
    /// `rows` the row count, and `cell` draws one cell. Rows are separated by grid rules
    /// and highlight on hover.
    pub fn table(
        &mut self,
        weights: &[f32],
        header: &[&str],
        rows: usize,
        mut cell: impl FnMut(usize, usize, &mut Ui<'_>),
    ) {
        let style = self.style().clone();
        let total = weights.iter().sum::<f32>().max(1e-6);
        let width = self.available_width();
        let widths: Vec<f32> = weights.iter().map(|w| width * w / total).collect();
        let row_h = style.row_height;
        let xs: Vec<f32> = widths
            .iter()
            .scan(self.cursor().x, |x, w| {
                let start = *x;
                *x += w;
                Some(start)
            })
            .collect();
        let p = style.palette;
        let gap = self.gap();
        self.set_gap(0.0);
        if !header.is_empty() {
            let row = self.allocate_row(row_h);
            self.gui().paint_rect(
                Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0),
                p.line,
            );
            for (i, h) in header.iter().enumerate() {
                let x = xs.get(i).copied().unwrap_or(row.min.x);
                let measured = self.gui().measure(h, style.font_size_small);
                self.gui().paint_text(
                    h,
                    Vec2::new(x + 4.0, row.center().y - measured.y * 0.5),
                    style.font_size_small,
                    p.text_faint,
                );
            }
        }
        for r in 0..rows {
            let row = self.allocate_row(row_h);
            let hovered = self
                .interact(self.make_id(("row", r)), row, Sense::HOVER)
                .hovered;
            if hovered {
                self.gui().paint_rect(row, p.row_hover);
            }
            self.gui().paint_rect(
                Rect::new(row.min.x, row.max.y - 1.0, row.width(), 1.0),
                p.line_grid,
            );
            for (c, w) in widths.iter().enumerate() {
                let rect = Rect::new(xs[c] + 4.0, row.min.y, (w - 8.0).max(0.0), row.height());
                let id = self.make_id(("cell", r, c));
                let mut cu = self.child(id, rect, Dir::Horizontal);
                cu.set_gap(4.0);
                cu.set_align(super::types::Align::Center);
                cu.gui().push_clip(rect);
                cell(r, c, &mut cu);
                cu.gui().pop_clip();
            }
        }
        self.set_gap(gap);
    }
}

/// Draw `add` in a centred modal box when `gui.modal() == Some(id)`. The base layer is
/// blocked while it is open; `Escape` closes it.
pub fn modal<R>(
    gui: &mut Gui,
    id: Id,
    size: Vec2,
    add: impl FnOnce(&mut Ui<'_>) -> R,
) -> Option<R> {
    if gui.modal() != Some(id) {
        return None;
    }
    let style = gui.style().clone();
    let win = gui.size();
    let prev = gui.layer();
    gui.set_layer(LAYER_POPUP);
    gui.paint_rect(Rect::from_min_size(Vec2::ZERO, win), style.palette.dim);
    let size = size.min(win - Vec2::splat(16.0));
    let rect = Rect::from_min_size(((win - size) * 0.5).round(), size);
    gui.register_popup(rect);
    gui.paint_shadow(rect, Vec2::new(0.0, 10.0), 28.0, 0.45);
    gui.paint_rect(rect, style.palette.popup);
    gui.paint_rect_outline(rect, 1.0, style.palette.line_hard);
    let inner = rect.shrink2(Vec2::new(12.0, 10.0));
    let result = {
        let mut ui = Ui::new(gui, id.with("modal"), inner);
        gui_clip(&mut ui, inner, add)
    };
    gui.set_layer(prev);
    Some(result)
}

fn gui_clip<R>(ui: &mut Ui<'_>, rect: Rect, add: impl FnOnce(&mut Ui<'_>) -> R) -> R {
    ui.gui().push_clip(rect);
    let r = add(ui);
    ui.gui().pop_clip();
    r
}

/// A small right- or down-pointing chevron centred at `c` (11 px box, 1.5 px stroke).
pub fn paint_chevron(gui: &mut Gui, c: Vec2, open: bool, color: Color) {
    if open {
        gui.paint_line(
            c + Vec2::new(-3.5, -1.5),
            c + Vec2::new(0.0, 2.0),
            1.5,
            color,
        );
        gui.paint_line(
            c + Vec2::new(0.0, 2.0),
            c + Vec2::new(3.5, -1.5),
            1.5,
            color,
        );
    } else {
        gui.paint_line(
            c + Vec2::new(-1.5, -3.5),
            c + Vec2::new(2.0, 0.0),
            1.5,
            color,
        );
        gui.paint_line(
            c + Vec2::new(2.0, 0.0),
            c + Vec2::new(-1.5, 3.5),
            1.5,
            color,
        );
    }
}
