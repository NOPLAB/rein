//! Geometry and identity primitives shared by the widget layer.

use core::hash::{Hash, Hasher};
use std::hash::DefaultHasher;

use glam::Vec2;

/// An axis-aligned rectangle in logical pixels (`min` inclusive, `max` exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Top-left corner.
    pub min: Vec2,
    /// Bottom-right corner.
    pub max: Vec2,
}

impl Rect {
    /// From position and size.
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            min: Vec2::new(x, y),
            max: Vec2::new(x + w, y + h),
        }
    }

    /// From corners.
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// From top-left and size.
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    /// Left edge.
    pub fn x(&self) -> f32 {
        self.min.x
    }

    /// Top edge.
    pub fn y(&self) -> f32 {
        self.min.y
    }

    /// Width (never negative).
    pub fn width(&self) -> f32 {
        (self.max.x - self.min.x).max(0.0)
    }

    /// Height (never negative).
    pub fn height(&self) -> f32 {
        (self.max.y - self.min.y).max(0.0)
    }

    /// Size.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width(), self.height())
    }

    /// Centre.
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Whether `p` lies inside.
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.y >= self.min.y && p.x < self.max.x && p.y < self.max.y
    }

    /// Shrunk by `d` on every side.
    pub fn shrink(&self, d: f32) -> Self {
        self.shrink2(Vec2::splat(d))
    }

    /// Shrunk by `d.x` horizontally and `d.y` vertically on each side.
    pub fn shrink2(&self, d: Vec2) -> Self {
        let r = Self {
            min: self.min + d,
            max: self.max - d,
        };
        if r.max.x < r.min.x || r.max.y < r.min.y {
            Self {
                min: self.center(),
                max: self.center(),
            }
        } else {
            r
        }
    }

    /// Grown by `d` on every side.
    pub fn expand(&self, d: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(d),
            max: self.max + Vec2::splat(d),
        }
    }

    /// Intersection (empty rect at `self.min` when disjoint).
    pub fn intersect(&self, other: &Self) -> Self {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max).max(min);
        Self { min, max }
    }

    /// Whether the rect has no area.
    pub fn is_empty(&self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// Split off `h` pixels from the top: `(top, rest)`.
    pub fn split_top(&self, h: f32) -> (Self, Self) {
        let h = h.clamp(0.0, self.height());
        (
            Self::new(self.min.x, self.min.y, self.width(), h),
            Self::new(self.min.x, self.min.y + h, self.width(), self.height() - h),
        )
    }

    /// Split off `h` pixels from the bottom: `(rest, bottom)`.
    pub fn split_bottom(&self, h: f32) -> (Self, Self) {
        let h = h.clamp(0.0, self.height());
        (
            Self::new(self.min.x, self.min.y, self.width(), self.height() - h),
            Self::new(self.min.x, self.max.y - h, self.width(), h),
        )
    }

    /// Split off `w` pixels from the left: `(left, rest)`.
    pub fn split_left(&self, w: f32) -> (Self, Self) {
        let w = w.clamp(0.0, self.width());
        (
            Self::new(self.min.x, self.min.y, w, self.height()),
            Self::new(self.min.x + w, self.min.y, self.width() - w, self.height()),
        )
    }

    /// Split off `w` pixels from the right: `(rest, right)`.
    pub fn split_right(&self, w: f32) -> (Self, Self) {
        let w = w.clamp(0.0, self.width());
        (
            Self::new(self.min.x, self.min.y, self.width() - w, self.height()),
            Self::new(self.max.x - w, self.min.y, w, self.height()),
        )
    }

    /// Translated by `d`.
    pub fn translate(&self, d: Vec2) -> Self {
        Self {
            min: self.min + d,
            max: self.max + d,
        }
    }
}

/// Stable identity of a widget across frames: a hash of its label and its ancestors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(u64);

impl Id {
    /// Root id.
    pub const ROOT: Self = Self(0x9e37_79b9_7f4a_7c15);

    /// Hash any value into an id.
    pub fn new(source: impl Hash) -> Self {
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        Self(h.finish())
    }

    /// Child id: `self` combined with `child`.
    pub fn with(self, child: impl Hash) -> Self {
        let mut h = DefaultHasher::new();
        self.0.hash(&mut h);
        child.hash(&mut h);
        Self(h.finish())
    }

    /// Raw value.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// What a widget wants to react to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sense {
    /// Clicks (press + release inside).
    pub click: bool,
    /// Drags (press captures the pointer until release).
    pub drag: bool,
}

impl Sense {
    /// Hover only.
    pub const HOVER: Self = Self {
        click: false,
        drag: false,
    };
    /// Click.
    pub const CLICK: Self = Self {
        click: true,
        drag: false,
    };
    /// Drag (and click).
    pub const DRAG: Self = Self {
        click: true,
        drag: true,
    };
}

/// What happened to a widget this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    /// The widget's id.
    pub id: Id,
    /// The rect it occupied.
    pub rect: Rect,
    /// Pointer is over it (and not blocked by a popup).
    pub hovered: bool,
    /// Primary button went down on it this frame.
    pub pressed: bool,
    /// A click completed on it this frame.
    pub clicked: bool,
    /// Secondary (right) button click completed on it.
    pub secondary_clicked: bool,
    /// Two clicks in quick succession completed on it.
    pub double_clicked: bool,
    /// It holds the pointer (press capture) and the button is still down.
    pub dragged: bool,
    /// The capture ended this frame.
    pub drag_released: bool,
    /// Pointer motion since the last frame while dragged.
    pub drag_delta: Vec2,
    /// It has keyboard focus.
    pub focused: bool,
}

impl Response {
    /// A response for a widget that saw no interaction.
    pub fn none(id: Id, rect: Rect) -> Self {
        Self {
            id,
            rect,
            hovered: false,
            pressed: false,
            clicked: false,
            secondary_clicked: false,
            double_clicked: false,
            dragged: false,
            drag_released: false,
            drag_delta: Vec2::ZERO,
            focused: false,
        }
    }
}

/// Layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dir {
    /// Top to bottom.
    #[default]
    Vertical,
    /// Left to right.
    Horizontal,
}

/// Horizontal alignment inside an allocated width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Left / top.
    #[default]
    Start,
    /// Centre.
    Center,
    /// Right / bottom.
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_ops() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.size(), Vec2::new(100.0, 50.0));
        assert!(r.contains(Vec2::new(10.0, 20.0)));
        assert!(!r.contains(Vec2::new(110.0, 20.0)));
        let (top, rest) = r.split_top(10.0);
        assert_eq!(top.height(), 10.0);
        assert_eq!(rest.min.y, 30.0);
        let (rest, right) = r.split_right(30.0);
        assert_eq!(right.min.x, 80.0);
        assert_eq!(rest.width(), 70.0);
        assert!(r.shrink(100.0).is_empty());
        let i = r.intersect(&Rect::new(50.0, 0.0, 100.0, 30.0));
        assert_eq!(i, Rect::new(50.0, 20.0, 60.0, 10.0));
        assert!(r.intersect(&Rect::new(500.0, 500.0, 1.0, 1.0)).is_empty());
    }

    #[test]
    fn ids_are_stable_and_hierarchical() {
        assert_eq!(Id::new("a"), Id::new("a"));
        assert_ne!(Id::new("a"), Id::new("b"));
        assert_ne!(Id::ROOT.with("a"), Id::ROOT.with("b").with("a"));
        assert_eq!(
            Id::ROOT.with("a").with(1_u32),
            Id::ROOT.with("a").with(1_u32)
        );
    }
}
