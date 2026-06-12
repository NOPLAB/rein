//! Fly camera control
//!
//! Provides 6DoF free movement camera control.

use crate::renderer::viewer::Camera;
use crate::window::event::{Event, Key, MouseButton};
use glam::{Quat, Vec3};

/// Fly camera control with 6DoF free movement.
/// Movement is relative to camera orientation (like a spaceship).
pub struct FlyControl {
    /// Movement speed (units per second).
    pub move_speed: f32,
    /// Mouse sensitivity for rotation.
    pub mouse_sensitivity: f32,
    /// Roll speed (radians per second).
    pub roll_speed: f32,
    /// Sprint speed multiplier.
    pub sprint_multiplier: f32,

    // Orientation (stored as quaternion for full 6DoF)
    orientation: Quat,

    // Input state
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    move_up: bool,
    move_down: bool,
    roll_left: bool,
    roll_right: bool,
    sprint: bool,
    mouse_captured: bool,
}

impl FlyControl {
    /// Create a new fly control.
    pub fn new(move_speed: f32, mouse_sensitivity: f32) -> Self {
        Self {
            move_speed,
            mouse_sensitivity,
            roll_speed: 1.0,
            sprint_multiplier: 3.0,
            orientation: Quat::IDENTITY,
            move_forward: false,
            move_backward: false,
            move_left: false,
            move_right: false,
            move_up: false,
            move_down: false,
            roll_left: false,
            roll_right: false,
            sprint: false,
            mouse_captured: false,
        }
    }

    /// Initialize the control from a camera's current orientation.
    pub fn from_camera(camera: &Camera, move_speed: f32, mouse_sensitivity: f32) -> Self {
        let direction = (camera.target - camera.position).normalize();
        let orientation = Quat::from_rotation_arc(Vec3::NEG_Z, direction);

        let mut control = Self::new(move_speed, mouse_sensitivity);
        control.orientation = orientation;
        control
    }

    /// Handle events and update the camera.
    pub fn handle_events(&mut self, camera: &mut Camera, events: &mut [Event], delta_time: f32) {
        for event in events.iter_mut() {
            if event.is_handled() {
                continue;
            }

            match event {
                Event::KeyPress { key, .. } => {
                    self.handle_key_press(*key);
                    event.set_handled();
                }
                Event::KeyRelease { key, .. } => {
                    self.handle_key_release(*key);
                    event.set_handled();
                }
                Event::MousePress { button, .. } => {
                    if *button == MouseButton::Right {
                        self.mouse_captured = true;
                        event.set_handled();
                    }
                }
                Event::MouseRelease { button, .. } => {
                    if *button == MouseButton::Right {
                        self.mouse_captured = false;
                        event.set_handled();
                    }
                }
                Event::MouseMotion { delta, .. } if self.mouse_captured => {
                    self.handle_mouse_motion(delta.0, delta.1);
                    event.set_handled();
                }
                _ => {}
            }
        }

        // Apply roll from Q/E keys
        self.update_roll(delta_time);

        // Apply movement
        self.update_camera(camera, delta_time);
    }

    fn handle_key_press(&mut self, key: Key) {
        match key {
            Key::W | Key::Up => self.move_forward = true,
            Key::S | Key::Down => self.move_backward = true,
            Key::A | Key::Left => self.move_left = true,
            Key::D | Key::Right => self.move_right = true,
            Key::Space => self.move_up = true,
            Key::Control => self.move_down = true,
            Key::Q => self.roll_left = true,
            Key::E => self.roll_right = true,
            Key::Shift => self.sprint = true,
            _ => {}
        }
    }

    fn handle_key_release(&mut self, key: Key) {
        match key {
            Key::W | Key::Up => self.move_forward = false,
            Key::S | Key::Down => self.move_backward = false,
            Key::A | Key::Left => self.move_left = false,
            Key::D | Key::Right => self.move_right = false,
            Key::Space => self.move_up = false,
            Key::Control => self.move_down = false,
            Key::Q => self.roll_left = false,
            Key::E => self.roll_right = false,
            Key::Shift => self.sprint = false,
            _ => {}
        }
    }

    fn handle_mouse_motion(&mut self, dx: f32, dy: f32) {
        // Yaw (rotate around global up)
        let yaw = Quat::from_rotation_y(-dx * self.mouse_sensitivity);
        // Pitch (rotate around local right)
        let pitch = Quat::from_rotation_x(-dy * self.mouse_sensitivity);

        // Apply yaw globally, pitch locally
        self.orientation = yaw * self.orientation * pitch;
        self.orientation = self.orientation.normalize();
    }

    fn update_roll(&mut self, delta_time: f32) {
        let mut roll_amount = 0.0;
        if self.roll_left {
            roll_amount += self.roll_speed * delta_time;
        }
        if self.roll_right {
            roll_amount -= self.roll_speed * delta_time;
        }

        if roll_amount.abs() > 0.0 {
            let roll = Quat::from_rotation_z(roll_amount);
            self.orientation *= roll;
            self.orientation = self.orientation.normalize();
        }
    }

    fn update_camera(&self, camera: &mut Camera, delta_time: f32) {
        // Get local coordinate axes from orientation
        let forward = self.orientation * Vec3::NEG_Z;
        let right = self.orientation * Vec3::X;
        let up = self.orientation * Vec3::Y;

        // Calculate movement velocity in camera space
        let mut velocity = Vec3::ZERO;

        if self.move_forward {
            velocity += forward;
        }
        if self.move_backward {
            velocity -= forward;
        }
        if self.move_right {
            velocity += right;
        }
        if self.move_left {
            velocity -= right;
        }
        if self.move_up {
            velocity += up;
        }
        if self.move_down {
            velocity -= up;
        }

        // Normalize and apply speed
        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize();
        }

        let speed = if self.sprint {
            self.move_speed * self.sprint_multiplier
        } else {
            self.move_speed
        };

        camera.position += velocity * speed * delta_time;
        camera.target = camera.position + forward;
        camera.up = up;
    }

    /// Check if mouse is captured for look control.
    pub fn is_mouse_captured(&self) -> bool {
        self.mouse_captured
    }

    /// Get current orientation quaternion.
    pub fn orientation(&self) -> Quat {
        self.orientation
    }

    /// Set orientation from a quaternion.
    pub fn set_orientation(&mut self, orientation: Quat) {
        self.orientation = orientation.normalize();
    }
}

impl Default for FlyControl {
    fn default() -> Self {
        Self::new(5.0, 0.002)
    }
}
