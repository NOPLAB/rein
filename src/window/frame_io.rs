//! Frame input/output types
//!
//! Types for passing data to and from the render loop callback.

use crate::context::WgpuContext;
use crate::core::texture::DepthTexture;
use crate::window::event::Event;

/// Viewport information.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    /// Get the aspect ratio.
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

/// Input data for a frame.
pub struct FrameInput<'a> {
    /// Events that occurred since the last frame.
    pub events: Vec<Event>,
    /// Time elapsed since the start of the application in seconds.
    pub elapsed_time: f64,
    /// Time elapsed since the last frame in seconds.
    pub delta_time: f64,
    /// The viewport dimensions.
    pub viewport: Viewport,
    /// The wgpu context.
    pub ctx: &'a WgpuContext,
    /// The surface texture view to render to.
    pub surface_view: &'a wgpu::TextureView,
    /// The depth texture.
    pub depth_texture: &'a DepthTexture,
    /// The surface format.
    pub surface_format: wgpu::TextureFormat,
}

impl FrameInput<'_> {
    /// Get the viewport width.
    pub fn width(&self) -> u32 {
        self.viewport.width
    }

    /// Get the viewport height.
    pub fn height(&self) -> u32 {
        self.viewport.height
    }

    /// Get the aspect ratio.
    pub fn aspect(&self) -> f32 {
        self.viewport.aspect()
    }

    /// Get the surface format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }
}

/// Output data from a frame.
#[derive(Debug, Clone, Default)]
pub struct FrameOutput {
    /// Whether to exit the application.
    pub exit: bool,
}

impl FrameOutput {
    /// Create a new frame output that doesn't exit.
    pub fn new() -> Self {
        Self { exit: false }
    }

    /// Create a frame output that exits the application.
    pub fn exit() -> Self {
        Self { exit: true }
    }
}
