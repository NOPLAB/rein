//! URDF loader
//!
//! Parses URDF files and extracts visual geometry information.

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3};
use std::path::Path;
use tracing::info;

/// Visual geometry from a URDF link.
#[derive(Debug, Clone)]
pub struct LinkVisual {
    /// Link name.
    pub link_name: String,
    /// Geometry type.
    pub geometry: GeometryType,
    /// Local transform from link origin.
    pub local_transform: Mat4,
    /// Color (RGB).
    pub color: [f32; 3],
}

/// Geometry types from URDF.
#[derive(Debug, Clone)]
pub enum GeometryType {
    Box { size: [f32; 3] },
    Cylinder { radius: f32, height: f32 },
    Sphere { radius: f32 },
    Capsule { radius: f32, length: f32 },
}

/// Joint information from URDF.
#[derive(Debug, Clone)]
pub struct JointInfo {
    /// Joint name.
    pub name: String,
    /// Parent link name.
    pub parent_link: String,
    /// Child link name.
    pub child_link: String,
    /// Joint origin transform.
    pub origin: Mat4,
    /// Rotation axis.
    pub axis: Vec3,
}

/// Loaded URDF model data.
pub struct UrdfModel {
    /// Visual geometry for each link.
    pub link_visuals: Vec<LinkVisual>,
    /// Joint information.
    pub joints: Vec<JointInfo>,
}

/// URDF loader utility.
pub struct UrdfLoader;

impl UrdfLoader {
    /// Load a URDF file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<UrdfModel> {
        let path = path.as_ref();
        info!("Loading URDF from {:?}", path);

        let robot = urdf_rs::read_file(path)
            .with_context(|| format!("Failed to load URDF from {}", path.display()))?;

        let mut link_visuals = Vec::new();
        let mut joints = Vec::new();

        // Color scheme
        let left_color = [0.2, 0.6, 0.8]; // Blue-ish
        let right_color = [0.8, 0.4, 0.2]; // Orange-ish
        let base_color = [0.5, 0.5, 0.5]; // Gray

        // Process links
        for link in &robot.links {
            for visual in &link.visual {
                let geometry = match &visual.geometry {
                    urdf_rs::Geometry::Box { size } => GeometryType::Box {
                        size: [size[0] as f32, size[1] as f32, size[2] as f32],
                    },
                    urdf_rs::Geometry::Cylinder { radius, length } => GeometryType::Cylinder {
                        radius: *radius as f32,
                        height: *length as f32,
                    },
                    urdf_rs::Geometry::Sphere { radius } => GeometryType::Sphere {
                        radius: *radius as f32,
                    },
                    urdf_rs::Geometry::Capsule { radius, length } => GeometryType::Capsule {
                        radius: *radius as f32,
                        length: *length as f32,
                    },
                    urdf_rs::Geometry::Mesh { .. } => {
                        // Skip mesh files for now
                        continue;
                    }
                };

                let color = Self::get_link_color(&link.name, left_color, right_color, base_color);
                let local_transform = Self::pose_to_mat4(&visual.origin);

                link_visuals.push(LinkVisual {
                    link_name: link.name.clone(),
                    geometry,
                    local_transform,
                    color,
                });
            }
        }

        // Process joints
        for joint in &robot.joints {
            let origin = Self::pose_to_mat4(&joint.origin);
            let axis = Vec3::new(
                joint.axis.xyz[0] as f32,
                joint.axis.xyz[1] as f32,
                joint.axis.xyz[2] as f32,
            );

            joints.push(JointInfo {
                name: joint.name.clone(),
                parent_link: joint.parent.link.clone(),
                child_link: joint.child.link.clone(),
                origin,
                axis,
            });
        }

        info!(
            "Loaded {} link visuals, {} joints",
            link_visuals.len(),
            joints.len()
        );

        Ok(UrdfModel {
            link_visuals,
            joints,
        })
    }

    fn get_link_color(name: &str, left: [f32; 3], right: [f32; 3], base: [f32; 3]) -> [f32; 3] {
        if name.starts_with("left") {
            left
        } else if name.starts_with("right") {
            right
        } else {
            base
        }
    }

    fn pose_to_mat4(pose: &urdf_rs::Pose) -> Mat4 {
        let translation = Vec3::new(pose.xyz[0] as f32, pose.xyz[1] as f32, pose.xyz[2] as f32);

        // URDF uses RPY (roll, pitch, yaw)
        let roll = pose.rpy[0] as f32;
        let pitch = pose.rpy[1] as f32;
        let yaw = pose.rpy[2] as f32;

        let rotation = Quat::from_euler(glam::EulerRot::XYZ, roll, pitch, yaw);

        Mat4::from_rotation_translation(rotation, translation)
    }
}
