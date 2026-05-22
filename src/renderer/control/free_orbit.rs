//! Free orbit camera control
//!
//! Provides orbit camera control without up-axis constraint,
//! allowing full 360-degree rotation in all directions.

use crate::renderer::viewer::Camera;
use crate::window::event::{Event, MouseButton};
use glam::{Quat, Vec3};

/// Free orbit camera control without up-axis constraint.
///
/// Unlike [`OrbitControl`](super::OrbitControl) which constrains rotation to prevent
/// flipping past the poles, `FreeOrbitControl` allows unrestricted rotation around
/// the target point, similar to three-d's `FreeOrbitControl`.
pub struct FreeOrbitControl {
    /// The point to orbit around.
    pub target: Vec3,
    /// Minimum distance from target.
    pub min_distance: f32,
    /// Maximum distance from target.
    pub max_distance: f32,
    /// Rotation speed multiplier.
    pub rotate_speed: f32,
    /// Zoom speed multiplier.
    pub zoom_speed: f32,
    /// Pan speed multiplier.
    pub pan_speed: f32,

    // Internal state
    orientation: Quat,
    distance: f32,
    left_mouse_pressed: bool,
    right_mouse_pressed: bool,
    middle_mouse_pressed: bool,
}

impl FreeOrbitControl {
    /// Create a new free orbit control.
    pub fn new(target: Vec3, min_distance: f32, max_distance: f32) -> Self {
        Self {
            target,
            min_distance,
            max_distance,
            rotate_speed: 0.005,
            zoom_speed: 0.1,
            pan_speed: 0.002,
            orientation: Quat::IDENTITY,
            distance: f32::midpoint(min_distance, max_distance),
            left_mouse_pressed: false,
            right_mouse_pressed: false,
            middle_mouse_pressed: false,
        }
    }

    /// Initialize from an existing camera's state.
    pub fn from_camera(camera: &Camera, min_distance: f32, max_distance: f32) -> Self {
        let to_camera = camera.position - camera.target;
        let distance = to_camera.length().clamp(min_distance, max_distance);
        let direction = to_camera.normalize();
        let orientation = Quat::from_rotation_arc(Vec3::Z, direction);

        Self {
            target: camera.target,
            min_distance,
            max_distance,
            rotate_speed: 0.005,
            zoom_speed: 0.1,
            pan_speed: 0.002,
            orientation,
            distance,
            left_mouse_pressed: false,
            right_mouse_pressed: false,
            middle_mouse_pressed: false,
        }
    }

    /// Handle events and update the camera.
    pub fn handle_events(&mut self, camera: &mut Camera, events: &mut [Event]) {
        for event in events.iter_mut() {
            if event.is_handled() {
                continue;
            }

            match event {
                Event::MousePress { button, .. } => match button {
                    MouseButton::Left => self.left_mouse_pressed = true,
                    MouseButton::Right => self.right_mouse_pressed = true,
                    MouseButton::Middle => self.middle_mouse_pressed = true,
                },
                Event::MouseRelease { button, .. } => match button {
                    MouseButton::Left => self.left_mouse_pressed = false,
                    MouseButton::Right => self.right_mouse_pressed = false,
                    MouseButton::Middle => self.middle_mouse_pressed = false,
                },
                Event::MouseMotion { delta, .. } => {
                    if self.left_mouse_pressed {
                        self.rotate(delta.0, delta.1);
                        self.update_camera(camera);
                        event.set_handled();
                    } else if self.right_mouse_pressed || self.middle_mouse_pressed {
                        self.pan(camera, delta.0, delta.1);
                        self.update_camera(camera);
                        event.set_handled();
                    }
                }
                Event::MouseWheel { delta, .. } => {
                    self.zoom(delta.1);
                    self.update_camera(camera);
                    event.set_handled();
                }
                _ => {}
            }
        }
    }

    /// Rotate freely (no up-axis constraint).
    fn rotate(&mut self, dx: f32, dy: f32) {
        // Apply yaw and pitch as quaternion rotations
        let yaw = Quat::from_rotation_y(-dx * self.rotate_speed);
        let pitch = Quat::from_rotation_x(-dy * self.rotate_speed);

        // Apply yaw globally, pitch locally
        self.orientation = yaw * self.orientation * pitch;
        self.orientation = self.orientation.normalize();
    }

    /// Zoom in/out.
    fn zoom(&mut self, delta: f32) {
        self.distance =
            (self.distance - delta * self.zoom_speed).clamp(self.min_distance, self.max_distance);
    }

    /// Pan the camera and target together.
    fn pan(&mut self, camera: &Camera, dx: f32, dy: f32) {
        let forward = (self.target - camera.position).normalize();
        let right = forward.cross(camera.up).normalize();
        let up = right.cross(forward).normalize();

        let pan_world = right * (-dx * self.pan_speed * self.distance)
            + up * (dy * self.pan_speed * self.distance);

        self.target += pan_world;
    }

    /// Update camera position from current state.
    fn update_camera(&self, camera: &mut Camera) {
        let offset = self.orientation * (Vec3::Z * self.distance);
        camera.position = self.target + offset;
        camera.target = self.target;
        // Keep up vector aligned with orientation
        camera.up = self.orientation * Vec3::Y;
    }

    /// Set the target point.
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
    }

    /// Get the current distance from target.
    pub fn distance(&self) -> f32 {
        self.distance
    }
}

impl Default for FreeOrbitControl {
    fn default() -> Self {
        Self::new(Vec3::ZERO, 0.5, 20.0)
    }
}
