//! Orbit camera with a configurable up axis, orthographic toggle and picking rays.

use glam::{Mat4, Vec2, Vec3, Vec4};

use crate::renderer::viewer::Viewer;
#[cfg(feature = "window")]
use crate::window::event::{Event, MouseButton};
#[cfg(feature = "window")]
use crate::window::frame_io::Viewport;

use super::pick::Ray;

/// Orbit camera: `eye` looks at `target`, orbits about `up`.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbitCamera {
    /// Camera position.
    pub eye: Vec3,
    /// Look-at point (and orbit centre).
    pub target: Vec3,
    /// World up axis (unit).
    pub up: Vec3,
    /// Vertical field of view in degrees (perspective).
    pub fov_y: f32,
    /// Near plane.
    pub near: f32,
    /// Far plane.
    pub far: f32,
    /// Orthographic instead of perspective. The ortho frustum height is derived from the
    /// eye distance and `fov_y`, so toggling keeps the framing at the target plane.
    pub ortho: bool,
    /// Zoom factor per wheel notch (`> 1`).
    pub zoom_step: f32,
    /// Orbit speed: full turns per viewport height of drag (`1.0` = one turn, the
    /// three.js `OrbitControls` default).
    pub rotate_speed: f32,
    /// Minimum eye distance.
    pub min_distance: f32,
    /// Maximum eye distance.
    pub max_distance: f32,
    /// Viewport in pixels (`width`, `height`). Set every frame.
    pub viewport_size: Vec2,
    /// Viewport origin in pixels (`x`, `y`) for sub-rect rendering.
    pub viewport_origin: Vec2,
    #[cfg(feature = "window")]
    dragging: Option<Drag>,
}

#[cfg(feature = "window")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    Orbit,
    Pan,
    Dolly,
}

impl OrbitCamera {
    /// A camera at `eye` looking at `target` with the given up axis.
    pub fn new(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        Self {
            eye,
            target,
            up: up.normalize_or_zero(),
            fov_y: 45.0,
            near: 0.01,
            far: 500.0,
            ortho: false,
            zoom_step: 1.1,
            rotate_speed: 1.0,
            min_distance: 0.05,
            max_distance: 1000.0,
            viewport_size: Vec2::new(1.0, 1.0),
            viewport_origin: Vec2::ZERO,
            #[cfg(feature = "window")]
            dragging: None,
        }
    }

    /// Distance from the eye to the target.
    pub fn distance(&self) -> f32 {
        (self.eye - self.target).length()
    }

    /// Unit vector from the target to the eye.
    pub fn direction(&self) -> Vec3 {
        (self.eye - self.target).normalize_or(self.up.any_orthonormal_vector())
    }

    /// Width / height.
    pub fn aspect(&self) -> f32 {
        self.viewport_size.x / self.viewport_size.y.max(1.0)
    }

    /// Set the viewport (pixels).
    pub fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.viewport_origin = Vec2::new(x, y);
        self.viewport_size = Vec2::new(width.max(1.0), height.max(1.0));
    }

    /// Ortho frustum height at the target plane.
    pub fn ortho_height(&self) -> f32 {
        2.0 * self.distance() * (self.fov_y.to_radians() * 0.5).tan()
    }

    /// Move the eye along its current direction so that a sphere fits the view, keeping
    /// the view direction.
    pub fn fit_sphere(&mut self, centre: Vec3, radius: f32) {
        let radius = radius.max(1e-3);
        let dir = self.direction();
        let fov_y = self.fov_y.to_radians();
        let fov_x = 2.0 * ((fov_y * 0.5).tan() * self.aspect()).atan();
        let fov = fov_y.min(fov_x);
        let dist = (radius / (fov * 0.5).sin()).clamp(self.min_distance, self.max_distance);
        self.target = centre;
        self.eye = centre + dir * dist;
    }

    /// Rotate about the target: `yaw` about `up`, `pitch` about the screen-right axis.
    /// Pitch is clamped short of the poles.
    pub fn orbit(&mut self, yaw: f32, pitch: f32) {
        let offset = self.eye - self.target;
        let dist = offset.length();
        if dist <= 0.0 {
            return;
        }
        let dir = offset / dist;
        let up = self.up;
        // Current elevation above the plane perpendicular to `up`.
        let elevation = dir.dot(up).clamp(-1.0, 1.0).asin();
        let max = core::f32::consts::FRAC_PI_2 - 0.01;
        let new_elevation = (elevation + pitch).clamp(-max, max);
        let horizontal = (dir - up * dir.dot(up)).normalize_or(up.any_orthonormal_vector());
        let rotated = Mat4::from_axis_angle(up, yaw).transform_vector3(horizontal);
        let new_dir = rotated * new_elevation.cos() + up * new_elevation.sin();
        self.eye = self.target + new_dir * dist;
    }

    /// Translate eye and target together by `pixels` of screen motion.
    pub fn pan(&mut self, pixels: Vec2) {
        let height = self.ortho_height();
        let per_px = height / self.viewport_size.y.max(1.0);
        let right = self.right();
        let up = right.cross(self.forward()).normalize_or(self.up);
        let delta = right * (-pixels.x * per_px) + up * (pixels.y * per_px);
        self.eye += delta;
        self.target += delta;
    }

    /// Zoom by `notches` wheel steps (positive = closer).
    pub fn zoom(&mut self, notches: f32) {
        let factor = self.zoom_step.max(1.0001).powf(-notches);
        let dist = (self.distance() * factor).clamp(self.min_distance, self.max_distance);
        self.eye = self.target + self.direction() * dist;
    }

    /// Unit forward vector.
    pub fn forward(&self) -> Vec3 {
        -self.direction()
    }

    /// Unit right vector (screen +x).
    pub fn right(&self) -> Vec3 {
        self.forward()
            .cross(self.up)
            .normalize_or(self.up.any_orthonormal_vector())
    }

    /// Safe up for the look-at matrix (never parallel to the view direction).
    fn view_up(&self) -> Vec3 {
        let f = self.forward();
        if f.cross(self.up).length_squared() < 1e-8 {
            self.up.any_orthonormal_vector()
        } else {
            self.up
        }
    }

    /// World-space ray through a pixel (window coordinates, origin top-left).
    pub fn screen_ray(&self, px: f32, py: f32) -> Ray {
        let local = Vec2::new(px, py) - self.viewport_origin;
        let ndc = Vec2::new(
            local.x / self.viewport_size.x * 2.0 - 1.0,
            1.0 - local.y / self.viewport_size.y * 2.0,
        );
        let inv = (self.projection_matrix() * self.view_matrix()).inverse();
        let unproject = |z: f32| {
            let p = inv * Vec4::new(ndc.x, ndc.y, z, 1.0);
            p.truncate() / p.w
        };
        let near = unproject(0.0);
        let far = unproject(1.0);
        if self.ortho {
            Ray::new(near, far - near)
        } else {
            Ray::new(self.eye, far - near)
        }
    }

    /// Project a world point to window pixels; `None` when behind the camera. The third
    /// component is normalized depth.
    pub fn project(&self, world: Vec3) -> Option<Vec3> {
        let clip = self.projection_matrix() * self.view_matrix() * world.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(Vec3::new(
            self.viewport_origin.x + (ndc.x + 1.0) * 0.5 * self.viewport_size.x,
            self.viewport_origin.y + (1.0 - ndc.y) * 0.5 * self.viewport_size.y,
            ndc.z,
        ))
    }

    /// Pixels per world unit at `distance` in front of the camera.
    pub fn pixels_per_unit(&self, distance: f32) -> f32 {
        let height = if self.ortho {
            self.ortho_height()
        } else {
            2.0 * distance.max(1e-4) * (self.fov_y.to_radians() * 0.5).tan()
        };
        self.viewport_size.y / height
    }

    /// Consume mouse events: left drag orbits, right drag pans, middle drag dollies,
    /// wheel zooms (the three.js `OrbitControls` mapping — the scene follows the pointer:
    /// dragging down brings the top of the model into view). Events already marked
    /// handled are skipped; consumed ones are marked handled.
    #[cfg(feature = "window")]
    pub fn handle_events(&mut self, events: &mut [Event]) {
        for event in events.iter_mut() {
            if event.is_handled() {
                continue;
            }
            match event {
                Event::MousePress {
                    button, position, ..
                } => {
                    if !self.contains(position.0, position.1) {
                        continue;
                    }
                    self.dragging = Some(match button {
                        MouseButton::Left => Drag::Orbit,
                        MouseButton::Right => Drag::Pan,
                        MouseButton::Middle => Drag::Dolly,
                    });
                    event.set_handled();
                }
                Event::MouseRelease { .. } => {
                    if self.dragging.take().is_some() {
                        event.set_handled();
                    }
                }
                Event::MouseMotion { delta, .. } => match self.dragging {
                    Some(Drag::Orbit) => {
                        let per_px = core::f32::consts::TAU * self.rotate_speed
                            / self.viewport_size.y.max(1.0);
                        self.orbit(-delta.0 * per_px, delta.1 * per_px);
                        event.set_handled();
                    }
                    Some(Drag::Pan) => {
                        self.pan(Vec2::new(delta.0, delta.1));
                        event.set_handled();
                    }
                    Some(Drag::Dolly) => {
                        // 100 px of drag = one wheel notch (three.js `dollyDelta * 0.01`);
                        // dragging down moves away.
                        self.zoom(-delta.1 * 0.01);
                        event.set_handled();
                    }
                    None => {}
                },
                Event::MouseWheel {
                    delta, position, ..
                } => {
                    if !self.contains(position.0, position.1) {
                        continue;
                    }
                    // `Window` scales line deltas by 20 px per notch.
                    self.zoom(delta.1 / 20.0);
                    event.set_handled();
                }
                _ => {}
            }
        }
    }

    /// Whether a pixel lies inside the viewport rect.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let p = Vec2::new(px, py) - self.viewport_origin;
        p.x >= 0.0 && p.y >= 0.0 && p.x < self.viewport_size.x && p.y < self.viewport_size.y
    }

    /// Whether a drag is in progress.
    #[cfg(feature = "window")]
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }
}

impl Viewer for OrbitCamera {
    fn position(&self) -> Vec3 {
        self.eye
    }

    fn view_matrix(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye, self.target, self.view_up())
    }

    fn projection_matrix(&self) -> Mat4 {
        if self.ortho {
            let h = self.ortho_height();
            let w = h * self.aspect();
            // Ortho keeps the same eye position; push the near plane back so objects
            // between the eye and the target plane are not clipped when zoomed in.
            glam::camera::rh::proj::directx::orthographic(
                -w * 0.5,
                w * 0.5,
                -h * 0.5,
                h * 0.5,
                -self.far,
                self.far,
            )
        } else {
            glam::camera::rh::proj::directx::perspective(
                self.fov_y.to_radians(),
                self.aspect(),
                self.near,
                self.far,
            )
        }
    }

    #[cfg(feature = "window")]
    fn viewport(&self) -> Viewport {
        Viewport {
            x: self.viewport_origin.x as u32,
            y: self.viewport_origin.y as u32,
            width: self.viewport_size.x as u32,
            height: self.viewport_size.y as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> OrbitCamera {
        let mut c = OrbitCamera::new(Vec3::new(3.0, -3.0, 2.0), Vec3::ZERO, Vec3::Z);
        c.set_viewport(0.0, 0.0, 800.0, 600.0);
        c
    }

    #[test]
    fn orbit_keeps_distance_and_respects_up() {
        let mut c = cam();
        let d0 = c.distance();
        c.orbit(1.0, 0.3);
        assert!((c.distance() - d0).abs() < 1e-4);
        // Pitching far past the pole clamps instead of flipping.
        c.orbit(0.0, 10.0);
        assert!(c.direction().dot(Vec3::Z) < 1.0);
        assert!(c.direction().dot(Vec3::Z) > 0.99);
        let m = c.view_matrix();
        assert!(m.is_finite());
    }

    /// Dragging down raises the eye (the scene follows the pointer, as in three.js
    /// `OrbitControls`); dragging right moves the eye clockwise seen from above.
    #[cfg(feature = "window")]
    #[test]
    fn drag_follows_the_pointer() {
        use crate::window::event::{Event, Modifiers, MouseButton};
        let mut c = cam();
        c.set_viewport(0.0, 0.0, 800.0, 600.0);
        c.eye = Vec3::new(5.0, 0.0, 0.0);
        c.target = Vec3::ZERO;
        let before = c.eye;
        let mut events = vec![
            Event::MousePress {
                button: MouseButton::Left,
                position: (400.0, 300.0),
                modifiers: Modifiers::default(),
                handled: false,
            },
            Event::MouseMotion {
                delta: (30.0, 30.0),
                position: (430.0, 330.0),
                modifiers: Modifiers::default(),
                handled: false,
            },
        ];
        c.handle_events(&mut events);
        assert!(c.eye.dot(c.up) > before.dot(c.up), "drag down → eye rises");
        // Clockwise from above about +up: +x rotates toward -y.
        assert!(c.eye.y < 0.0, "drag right → clockwise: {:?}", c.eye);
        assert!(events.iter().all(Event::is_handled));
    }

    #[test]
    fn zoom_and_pan() {
        let mut c = cam();
        let d0 = c.distance();
        c.zoom(1.0);
        assert!(c.distance() < d0);
        c.zoom(-1.0);
        assert!((c.distance() - d0).abs() < 1e-4);
        let t0 = c.target;
        c.pan(Vec2::new(100.0, 0.0));
        assert!((c.target - t0).length() > 0.0);
        assert!((c.distance() - d0).abs() < 1e-4);
    }

    #[test]
    fn project_and_unproject_round_trip() {
        for ortho in [false, true] {
            let mut c = cam();
            c.ortho = ortho;
            let p = Vec3::new(0.2, 0.1, 0.5);
            let s = c.project(p).unwrap();
            let ray = c.screen_ray(s.x, s.y);
            // The ray passes within a millimetre of the point.
            let t = (p - ray.origin).dot(ray.direction);
            assert!((ray.at(t) - p).length() < 1e-3, "ortho={ortho}");
        }
        // Behind the camera is None.
        let c = cam();
        assert!(c.project(c.eye + c.direction() * 5.0).is_none());
    }

    #[test]
    fn fit_sphere_frames_it() {
        let mut c = cam();
        c.fit_sphere(Vec3::new(1.0, 1.0, 1.0), 0.5);
        assert_eq!(c.target, Vec3::new(1.0, 1.0, 1.0));
        // Sphere edge must project inside the viewport.
        let edge = c.target + c.right() * 0.5;
        let s = c.project(edge).unwrap();
        assert!(s.x > 0.0 && s.x < 800.0);
    }
}
