//! Orbit camera control
//!
//! Provides orbit camera control that rotates around a target point.

use crate::renderer::viewer::Camera;
use crate::window::event::{Event, MouseButton};
use glam::Vec3;

/// Orbit camera control that rotates around a target point.
pub struct OrbitControl {
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
    left_mouse_pressed: bool,
    right_mouse_pressed: bool,
    middle_mouse_pressed: bool,
}

impl OrbitControl {
    /// Create a new orbit control.
    pub fn new(target: Vec3, min_distance: f32, max_distance: f32) -> Self {
        Self {
            target,
            min_distance,
            max_distance,
            rotate_speed: 0.005,
            zoom_speed: 0.1,
            pan_speed: 0.002,
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
                        self.rotate(camera, delta.0, delta.1);
                        event.set_handled();
                    } else if self.right_mouse_pressed || self.middle_mouse_pressed {
                        self.pan(camera, delta.0, delta.1);
                        event.set_handled();
                    }
                }
                Event::MouseWheel { delta, .. } => {
                    self.zoom(camera, delta.1);
                    event.set_handled();
                }
                _ => {}
            }
        }
    }

    /// Rotate the camera around the target.
    fn rotate(&self, camera: &mut Camera, dx: f32, dy: f32) {
        let to_camera = camera.position - self.target;
        let distance = to_camera.length();

        // Calculate current spherical coordinates
        let theta = to_camera.x.atan2(to_camera.z);
        let phi = (to_camera.y / distance).acos();

        // Apply rotation
        let new_theta = theta - dx * self.rotate_speed;
        let new_phi = (phi - dy * self.rotate_speed).clamp(0.01, std::f32::consts::PI - 0.01);

        // Convert back to Cartesian
        let sin_phi = new_phi.sin();
        camera.position = self.target
            + Vec3::new(
                distance * sin_phi * new_theta.sin(),
                distance * new_phi.cos(),
                distance * sin_phi * new_theta.cos(),
            );

        camera.target = self.target;
    }

    /// Zoom the camera in/out.
    fn zoom(&self, camera: &mut Camera, delta: f32) {
        let to_camera = camera.position - self.target;
        let distance = to_camera.length();
        let direction = to_camera.normalize();

        let new_distance =
            (distance - delta * self.zoom_speed).clamp(self.min_distance, self.max_distance);

        camera.position = self.target + direction * new_distance;
    }

    /// Pan the camera.
    fn pan(&mut self, camera: &mut Camera, dx: f32, dy: f32) {
        let forward = (self.target - camera.position).normalize();
        let right = forward.cross(camera.up).normalize();
        let up = right.cross(forward).normalize();

        let distance = (camera.position - self.target).length();
        let pan_amount = Vec3::new(
            -dx * self.pan_speed * distance,
            dy * self.pan_speed * distance,
            0.0,
        );

        let pan_world = right * pan_amount.x + up * pan_amount.y;

        camera.position += pan_world;
        camera.target += pan_world;
        self.target = camera.target;
    }

    /// Set the target point.
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
    }
}

impl Default for OrbitControl {
    fn default() -> Self {
        Self::new(Vec3::ZERO, 0.5, 20.0)
    }
}
