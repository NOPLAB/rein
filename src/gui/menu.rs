//! Menu bar with drop-down menus, and context menus.

use glam::Vec2;

use super::builder::Ui;
use super::frame::LAYER_POPUP;
use super::types::{Dir, Id, Rect, Sense};
use super::widgets::ButtonKind;

/// One open drop-down.
#[derive(Debug)]
pub struct Menu<'a, 'g> {
    ui: &'a mut Ui<'g>,
    id: Id,
    width: f32,
}

impl Menu<'_, '_> {
    /// A menu item. Returns `true` when clicked (the menu closes).
    pub fn item(&mut self, text: &str) -> bool {
        self.item_shortcut(text, "")
    }

    /// A menu item with a right-aligned hint (a shortcut key).
    pub fn item_shortcut(&mut self, text: &str, hint: &str) -> bool {
        let style = self.ui.style().clone();
        self.ui.set_next_width(self.width);
        let r = self.ui.button_kind(text, ButtonKind::Flat);
        if !hint.is_empty() {
            let m = self.ui.gui().measure(hint, style.font_size_small);
            let pos = Vec2::new(
                r.rect.max.x - style.padding - m.x,
                r.rect.center().y - m.y * 0.5,
            );
            self.ui
                .gui()
                .paint_text(hint, pos, style.font_size_small, style.palette.text_dim);
        }
        if r.clicked {
            self.ui.gui().close_popups();
            return true;
        }
        false
    }

    /// A checkable item; `checked` is toggled on click. Returns `true` when toggled.
    pub fn toggle(&mut self, text: &str, checked: &mut bool) -> bool {
        let style = self.ui.style().clone();
        let label = format!("{}  {text}", if *checked { "✓" } else { "  " });
        self.ui.set_next_width(self.width);
        let r = self.ui.button_kind(&label, ButtonKind::Flat);
        let _ = style;
        if r.clicked {
            *checked = !*checked;
            self.ui.gui().close_popups();
            return true;
        }
        false
    }

    /// A separator line.
    pub fn separator(&mut self) {
        self.ui.separator();
    }

    /// The id of this menu.
    pub fn id(&self) -> Id {
        self.id
    }
}

impl Ui<'_> {
    /// A horizontal menu bar. Call [`Ui::menu`] inside `add` for each drop-down.
    pub fn menu_bar(&mut self, add: impl FnOnce(&mut Ui<'_>)) {
        let style = self.style().clone();
        let row = self.allocate_row(style.menu_height);
        self.gui().paint_rect(row, style.palette.panel);
        let id = self.make_id("menu_bar");
        let mut bar = self.child(id, row.shrink2(Vec2::new(4.0, 0.0)), Dir::Horizontal);
        bar.set_gap(2.0);
        bar.set_align(super::types::Align::Center);
        add(&mut bar);
    }

    /// A drop-down `title` in a menu bar (or a stand-alone menu button). While another
    /// menu of the same bar is open, hovering this one switches to it.
    pub fn menu(&mut self, title: &str, add: impl FnOnce(&mut Menu<'_, '_>)) {
        let style = self.style().clone();
        let id = self.make_id(("menu", title));
        let bar_id = self.id();
        let r = self.button_kind(title, ButtonKind::Flat);
        let open = self.gui().is_open(id);
        if r.clicked {
            let was = self.gui().was_open(id);
            self.gui().close_popups();
            self.gui().set_open(id, !was);
        } else if r.hovered && !open && self.gui().any_popup_open() && self.sibling_open(bar_id) {
            self.gui().close_popups();
            self.gui().set_open(id, true);
        }
        if self.gui().is_open(id) {
            self.gui().paint_rect(r.rect, style.palette.panel_alt);
            let m = self.gui().measure(title, style.font_size);
            let pos = r.rect.min + (r.rect.size() - m) * 0.5;
            self.gui()
                .paint_text(title, pos, style.font_size, style.palette.text);
            // Remember which bar owns the open menu so siblings can take over on hover.
            self.gui().set_scalar(bar_id.with("open_menu"), 1.0);
            let width = 220.0_f32;
            let anchor = r.rect;
            let max_h = self.gui().size().y - anchor.max.y - 8.0;
            let rect = Rect::from_min_size(
                Vec2::new(anchor.min.x, anchor.max.y),
                Vec2::new(width, max_h.max(style.row_height)),
            );
            let prev = self.gui().layer();
            self.gui().set_layer(LAYER_POPUP);
            let handle = self.gui().reserve_rect();
            let border = self.gui().reserve_rect();
            let used = {
                let mut c = self.child(id.with("popup"), rect.shrink(1.0), Dir::Vertical);
                c.set_gap(0.0);
                let mut menu = Menu {
                    ui: &mut c,
                    id,
                    width: width - 2.0,
                };
                add(&mut menu);
                c.used_size()
            };
            let outer = Rect::from_min_size(rect.min, Vec2::new(width, used.y + 2.0));
            self.gui().fill_reserved(handle, outer, style.palette.popup);
            // Border as four thin rects is not possible with one reserved quad: draw a
            // 1px frame by filling the reserved quad slightly larger with the line colour.
            self.gui()
                .fill_reserved(border, outer.expand(1.0), style.palette.line);
            self.gui().register_popup(outer.expand(1.0));
            self.gui().set_layer(prev);
        } else {
            self.gui().set_scalar(bar_id.with("open_menu"), 0.0);
        }
    }

    /// Whether a sibling menu of `bar_id` was open on the previous frame.
    fn sibling_open(&mut self, bar_id: Id) -> bool {
        self.gui().scalar(bar_id.with("open_menu")) > 0.0
    }

    /// A context menu opened by a right click on `response`'s widget.
    pub fn context_menu(
        &mut self,
        response: &super::types::Response,
        add: impl FnOnce(&mut Menu<'_, '_>),
    ) {
        let id = response.id.with("context");
        if response.secondary_clicked {
            self.gui().close_popups();
            self.gui().set_open(id, true);
            let at = self.gui().mouse();
            self.gui().set_vec2(id, Some(at));
        }
        if !self.gui().is_open(id) {
            return;
        }
        let style = self.style().clone();
        let at = self.gui().vec2(id).unwrap_or(response.rect.min);
        let width = 200.0_f32;
        let prev = self.gui().layer();
        self.gui().set_layer(LAYER_POPUP);
        let border = self.gui().reserve_rect();
        let handle = self.gui().reserve_rect();
        let rect = Rect::from_min_size(at, Vec2::new(width, self.gui().size().y - at.y));
        let used = {
            let mut c = self.child(id.with("popup"), rect.shrink(1.0), Dir::Vertical);
            c.set_gap(0.0);
            let mut menu = Menu {
                ui: &mut c,
                id,
                width: width - 2.0,
            };
            add(&mut menu);
            c.used_size()
        };
        let outer = Rect::from_min_size(at, Vec2::new(width, used.y + 2.0));
        self.gui()
            .fill_reserved(border, outer.expand(1.0), style.palette.line);
        self.gui().fill_reserved(handle, outer, style.palette.popup);
        self.gui().register_popup(outer.expand(1.0));
        self.gui().set_layer(prev);
        let _ = Sense::CLICK;
    }
}
