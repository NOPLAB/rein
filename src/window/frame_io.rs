//! Frame input/output types
//!
//! Types for passing data to and from the render loop callback.

use crate::context::WgpuContext;
use crate::core::texture::DepthTexture;
use crate::window::event::{Cursor, Event};

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
    /// Whether the window is maximized (client-side title bars swap their glyph).
    pub maximized: bool,
    /// Window scale factor (physical pixels per logical pixel).
    pub scale_factor: f64,
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

/// Which window edge a client-side resize drag starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    /// Top.
    North,
    /// Bottom.
    South,
    /// Right.
    East,
    /// Left.
    West,
    /// Top-right corner.
    NorthEast,
    /// Top-left corner.
    NorthWest,
    /// Bottom-right corner.
    SouthEast,
    /// Bottom-left corner.
    SouthWest,
}

impl ResizeEdge {
    /// The cursor shape for this edge.
    pub const fn cursor(self) -> Cursor {
        match self {
            Self::North | Self::South => Cursor::NsResize,
            Self::East | Self::West => Cursor::EwResize,
            Self::NorthEast | Self::SouthWest => Cursor::NeswResize,
            Self::NorthWest | Self::SouthEast => Cursor::NwseResize,
        }
    }

    /// The winit resize direction.
    pub fn to_winit(self) -> winit::window::ResizeDirection {
        use winit::window::ResizeDirection as D;
        match self {
            Self::North => D::North,
            Self::South => D::South,
            Self::East => D::East,
            Self::West => D::West,
            Self::NorthEast => D::NorthEast,
            Self::NorthWest => D::NorthWest,
            Self::SouthEast => D::SouthEast,
            Self::SouthWest => D::SouthWest,
        }
    }
}

/// A window operation requested by the frame callback (client-side decorations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    /// Start a window move drag (call while the primary button is down).
    DragMove,
    /// Start a resize drag from `edge` (call while the primary button is down).
    DragResize(ResizeEdge),
    /// Minimize.
    Minimize,
    /// Maximize, or restore when already maximized.
    ToggleMaximize,
    /// Close the window (exits the loop).
    Close,
}

/// Output data from a frame.
#[derive(Debug, Clone, Default)]
pub struct FrameOutput {
    /// Whether to exit the application.
    pub exit: bool,
    /// Window operations to perform after this frame.
    pub window: Vec<WindowCommand>,
    /// Cursor shape for the next frame (`None` = leave as is).
    pub cursor: Option<Cursor>,
}

impl FrameOutput {
    /// Create a new frame output that doesn't exit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a frame output that exits the application.
    pub fn exit() -> Self {
        Self {
            exit: true,
            ..Self::default()
        }
    }

    /// Queue a window command.
    #[must_use]
    pub fn with_command(mut self, command: WindowCommand) -> Self {
        self.window.push(command);
        self
    }

    /// Set the cursor shape.
    #[must_use]
    pub fn with_cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }
}
