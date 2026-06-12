//! First-person camera control
//!
//! Provides WASD movement with mouse look.

use crate::renderer::viewer::Camera;
use crate::window::event::{Event, Key, MouseButton};
use glam::Vec3;

/// First-person camera control with WASD movement and mouse look.
pub struct FirstPersonControl {
    /// Movement speed (units per second).
    pub move_speed: f32,
    /// Mouse sensitivity for rotation.
    pub mouse_sensitivity: f32,
    /// Sprint speed multiplier (when holding shift).
    pub sprint_multiplier: f32,

    // Rotation state (pitch and yaw in radians)
    pitch: f32,
    yaw: f32,

    // Input state
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    move_up: bool,
    move_down: bool,
    sprint: bool,
    mouse_captured: bool,
}

impl FirstPersonControl {
    /// Create a new first-person control.
    pub fn new(move_speed: f32, mouse_sensitivity: f32) -> Self {
        Self {
            move_speed,
            mouse_sensitivity,
            sprint_multiplier: 2.0,
            pitch: 0.0,
            yaw: 0.0,
            move_forward: false,
            move_backward: false,
            move_left: false,
            move_right: false,
            move_up: false,
            move_down: false,
            sprint: false,
            mouse_captured: false,
        }
    }

    /// Initialize the control from a camera's current orientation.
    pub fn from_camera(camera: &Camera, move_speed: f32, mouse_sensitivity: f32) -> Self {
        let direction = (camera.target - camera.position).normalize();
        let pitch = direction.y.asin();
        let yaw = direction.x.atan2(direction.z);

        let mut control = Self::new(move_speed, mouse_sensitivity);
        control.pitch = pitch;
        control.yaw = yaw;
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
            Key::Shift => self.sprint = false,
            _ => {}
        }
    }

    fn handle_mouse_motion(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * self.mouse_sensitivity;
        self.pitch = (self.pitch - dy * self.mouse_sensitivity).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
    }

    fn update_camera(&self, camera: &mut Camera, delta_time: f32) {
        // Calculate forward and right vectors
        let forward = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
        .normalize();

        let right = forward.cross(Vec3::Y).normalize();
        let up = Vec3::Y;

        // Calculate movement velocity
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
    }

    /// Check if mouse is captured for look control.
    pub fn is_mouse_captured(&self) -> bool {
        self.mouse_captured
    }

    /// Get current pitch angle in radians.
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Get current yaw angle in radians.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
}

impl Default for FirstPersonControl {
    fn default() -> Self {
        Self::new(5.0, 0.002)
    }
}
