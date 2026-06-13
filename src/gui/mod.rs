//! GUI module
//!
//! Provides UI rendering capabilities including text and 2D primitives.
//!
//! [`PrimitiveRenderer`] and [`TextRenderer`] are window-independent and available
//! whenever the `gui` feature is on. [`UiContext`] is an immediate-mode widget layer
//! built on the window event types, so it additionally requires the `window` feature.

pub mod primitive;
pub mod text;

#[cfg(feature = "window")]
pub mod ui;

pub use primitive::PrimitiveRenderer;
pub use text::{TextBuilder, TextRenderer};

#[cfg(feature = "window")]
pub use ui::UiContext;
