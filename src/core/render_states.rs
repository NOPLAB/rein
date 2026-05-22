//! Render state configurations
//!
//! Provides convenient structs for configuring render pipeline states.

/// Clear state for render targets.
#[derive(Debug, Clone, Copy)]
pub struct ClearState {
    /// Color to clear to (RGBA), or None to not clear.
    pub color: Option<[f32; 4]>,
    /// Depth value to clear to (0.0-1.0), or None to not clear.
    pub depth: Option<f32>,
    /// Stencil value to clear to, or None to not clear.
    pub stencil: Option<u32>,
}

impl ClearState {
    /// Create a clear state that clears color only.
    pub fn color(color: [f32; 4]) -> Self {
        Self {
            color: Some(color),
            depth: None,
            stencil: None,
        }
    }

    /// Create a clear state that clears depth only.
    pub fn depth(depth: f32) -> Self {
        Self {
            color: None,
            depth: Some(depth),
            stencil: None,
        }
    }

    /// Create a clear state that clears both color and depth.
    pub fn color_and_depth(color: [f32; 4], depth: f32) -> Self {
        Self {
            color: Some(color),
            depth: Some(depth),
            stencil: None,
        }
    }

    /// Create a clear state that doesn't clear anything.
    pub fn none() -> Self {
        Self {
            color: None,
            depth: None,
            stencil: None,
        }
    }

    /// Get the wgpu load operation for color.
    pub fn color_load_op(&self) -> wgpu::LoadOp<wgpu::Color> {
        match self.color {
            Some([r, g, b, a]) => wgpu::LoadOp::Clear(wgpu::Color {
                r: f64::from(r),
                g: f64::from(g),
                b: f64::from(b),
                a: f64::from(a),
            }),
            None => wgpu::LoadOp::Load,
        }
    }

    /// Get the wgpu load operation for depth.
    pub fn depth_load_op(&self) -> wgpu::LoadOp<f32> {
        match self.depth {
            Some(d) => wgpu::LoadOp::Clear(d),
            None => wgpu::LoadOp::Load,
        }
    }

    /// Get the wgpu load operation for stencil.
    pub fn stencil_load_op(&self) -> wgpu::LoadOp<u32> {
        match self.stencil {
            Some(s) => wgpu::LoadOp::Clear(s),
            None => wgpu::LoadOp::Load,
        }
    }
}

impl Default for ClearState {
    fn default() -> Self {
        Self::color_and_depth([0.0, 0.0, 0.0, 1.0], 1.0)
    }
}

/// Blend state configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendState {
    /// No blending (opaque).
    #[default]
    Opaque,
    /// Standard alpha blending.
    Alpha,
    /// Additive blending.
    Additive,
    /// Pre-multiplied alpha blending.
    PremultipliedAlpha,
}

impl BlendState {
    /// Convert to wgpu blend state.
    pub fn to_wgpu(&self) -> Option<wgpu::BlendState> {
        match self {
            Self::Opaque => None,
            Self::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            Self::Additive => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            Self::PremultipliedAlpha => Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        }
    }
}

/// Depth test configuration.
#[derive(Debug, Clone, Copy)]
pub struct DepthState {
    /// Whether to write to the depth buffer.
    pub write: bool,
    /// Comparison function for depth test.
    pub compare: wgpu::CompareFunction,
}

impl DepthState {
    /// Depth testing enabled with writes.
    pub fn read_write() -> Self {
        Self {
            write: true,
            compare: wgpu::CompareFunction::Less,
        }
    }

    /// Depth testing enabled without writes.
    pub fn read_only() -> Self {
        Self {
            write: false,
            compare: wgpu::CompareFunction::Less,
        }
    }

    /// Depth testing disabled.
    pub fn disabled() -> Self {
        Self {
            write: false,
            compare: wgpu::CompareFunction::Always,
        }
    }

    /// No depth testing (alias for disabled).
    pub fn none() -> Self {
        Self::disabled()
    }

    /// Convert to wgpu depth stencil state.
    pub fn to_wgpu(&self, format: wgpu::TextureFormat) -> wgpu::DepthStencilState {
        wgpu::DepthStencilState {
            format,
            depth_write_enabled: self.write,
            depth_compare: self.compare,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }
    }
}

impl Default for DepthState {
    fn default() -> Self {
        Self::read_write()
    }
}

/// Cull mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CullState {
    /// No culling.
    None,
    /// Cull front faces.
    Front,
    /// Cull back faces.
    #[default]
    Back,
}

impl CullState {
    /// Convert to wgpu cull mode.
    pub fn to_wgpu(&self) -> Option<wgpu::Face> {
        match self {
            Self::None => None,
            Self::Front => Some(wgpu::Face::Front),
            Self::Back => Some(wgpu::Face::Back),
        }
    }
}
