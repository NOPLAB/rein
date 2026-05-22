//! Camera and viewer abstractions
//!
//! Provides camera types for 3D rendering.

#[cfg(feature = "window")]
use crate::window::frame_io::Viewport;
use glam::{Mat4, Vec3};

/// Projection mode for a camera.
#[derive(Debug, Clone, Copy)]
pub enum Projection {
    /// Perspective projection.
    Perspective {
        /// Field of view in radians.
        fov: f32,
        /// Aspect ratio (width / height).
        aspect: f32,
        /// Near clipping plane.
        near: f32,
        /// Far clipping plane.
        far: f32,
    },
    /// Orthographic projection.
    Orthographic {
        /// Width of the view.
        width: f32,
        /// Height of the view.
        height: f32,
        /// Near clipping plane.
        near: f32,
        /// Far clipping plane.
        far: f32,
    },
}

impl Projection {
    /// Create a perspective projection.
    pub fn perspective(fov_degrees: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self::Perspective {
            fov: fov_degrees.to_radians(),
            aspect,
            near,
            far,
        }
    }

    /// Create an orthographic projection.
    pub fn orthographic(width: f32, height: f32, near: f32, far: f32) -> Self {
        Self::Orthographic {
            width,
            height,
            near,
            far,
        }
    }

    /// Get the projection matrix.
    pub fn matrix(&self) -> Mat4 {
        match *self {
            Self::Perspective {
                fov,
                aspect,
                near,
                far,
            } => Mat4::perspective_rh(fov, aspect, near, far),
            Self::Orthographic {
                width,
                height,
                near,
                far,
            } => Mat4::orthographic_rh(
                -width / 2.0,
                width / 2.0,
                -height / 2.0,
                height / 2.0,
                near,
                far,
            ),
        }
    }

    /// Update the aspect ratio.
    pub fn set_aspect(&mut self, aspect: f32) {
        if let Self::Perspective { aspect: a, .. } = self {
            *a = aspect;
        }
    }
}

/// Trait for objects that can view a scene.
pub trait Viewer {
    /// Get the camera position.
    fn position(&self) -> Vec3;

    /// Get the view matrix.
    fn view_matrix(&self) -> Mat4;

    /// Get the projection matrix.
    fn projection_matrix(&self) -> Mat4;

    /// Get the combined view-projection matrix.
    fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Get the viewport.
    #[cfg(feature = "window")]
    fn viewport(&self) -> Viewport;
}

/// A 3D camera.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera position.
    pub position: Vec3,
    /// Point the camera is looking at.
    pub target: Vec3,
    /// Up vector.
    pub up: Vec3,
    /// Projection mode.
    pub projection: Projection,
    /// Viewport.
    #[cfg(feature = "window")]
    viewport: Viewport,
}

impl Camera {
    /// Create a new perspective camera.
    pub fn new_perspective(
        position: Vec3,
        target: Vec3,
        up: Vec3,
        fov_degrees: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            position,
            target,
            up,
            projection: Projection::perspective(fov_degrees, aspect, near, far),
            #[cfg(feature = "window")]
            viewport: Viewport {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        }
    }

    /// Create a new orthographic camera.
    pub fn new_orthographic(
        position: Vec3,
        target: Vec3,
        up: Vec3,
        width: f32,
        height: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            position,
            target,
            up,
            projection: Projection::orthographic(width, height, near, far),
            #[cfg(feature = "window")]
            viewport: Viewport {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        }
    }

    /// Set the viewport and update aspect ratio.
    #[cfg(feature = "window")]
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
        self.projection.set_aspect(viewport.aspect());
    }

    /// Get the forward direction (from camera to target).
    pub fn forward(&self) -> Vec3 {
        (self.target - self.position).normalize()
    }

    /// Get the right direction.
    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up).normalize()
    }
}

impl Viewer for Camera {
    fn position(&self) -> Vec3 {
        self.position
    }

    fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    fn projection_matrix(&self) -> Mat4 {
        self.projection.matrix()
    }

    #[cfg(feature = "window")]
    fn viewport(&self) -> Viewport {
        self.viewport
    }
}

/// Camera uniform data for GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// View-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// Camera eye position (w component unused).
    pub eye: [f32; 4],
}

impl CameraUniform {
    /// Create a new camera uniform from a viewer.
    pub fn from_viewer(viewer: &dyn Viewer) -> Self {
        let vp = viewer.view_projection_matrix();
        let pos = viewer.position();
        Self {
            view_proj: vp.to_cols_array_2d(),
            eye: [pos.x, pos.y, pos.z, 1.0],
        }
    }
}
