//! Robot model
//!
//! Provides a renderable robot model from URDF.

use crate::context::WgpuContext;
use crate::core::pipeline::Vertex;
use crate::renderer::geometry::{Aabb, Geometry, Mesh};
use crate::renderer::light::Light;
use crate::renderer::material::{ColorMaterial, GridMaterial, Material};
use crate::renderer::viewer::Viewer;
use crate::urdf::loader::{GeometryType, JointInfo, UrdfLoader};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use std::path::Path;
#[cfg(feature = "ecs")]
use std::sync::Arc;

/// A link in the robot model with its mesh and material.
struct RobotLink {
    name: String,
    mesh: Mesh,
    local_transform: Mat4,
    world_transform: Mat4,
}

/// A renderable robot model loaded from URDF.
pub struct RobotModel {
    links: Vec<RobotLink>,
    joints: Vec<JointInfo>,
    link_transforms: HashMap<String, Mat4>,
    material: ColorMaterial,
    grid_mesh: Mesh,
    grid_material: GridMaterial,
}

impl RobotModel {
    /// Load a robot model from a URDF file.
    pub fn from_urdf<P: AsRef<Path>>(
        ctx: &WgpuContext,
        path: P,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let urdf_model = UrdfLoader::load(path)?;

        let material = ColorMaterial::new(ctx, format)?;
        let grid_material = GridMaterial::new(ctx, format)?;

        let mut links = Vec::with_capacity(urdf_model.link_visuals.len());

        for visual in &urdf_model.link_visuals {
            let mesh = Self::create_mesh(ctx, &visual.geometry, visual.color);

            links.push(RobotLink {
                name: visual.link_name.clone(),
                mesh,
                local_transform: visual.local_transform,
                world_transform: Mat4::IDENTITY,
            });
        }

        let grid_mesh = Mesh::quad(ctx, 2.0, 2.0, [0.3, 0.3, 0.3]);

        let mut model = Self {
            links,
            joints: urdf_model.joints,
            link_transforms: HashMap::new(),
            material,
            grid_mesh,
            grid_material,
        };

        // Initialize transforms
        model.update_joints(&[], &[]);

        Ok(model)
    }

    /// Create a mesh from geometry type.
    fn create_mesh(ctx: &WgpuContext, geometry: &GeometryType, color: [f32; 3]) -> Mesh {
        match geometry {
            GeometryType::Box { size } => {
                Self::create_box_mesh(ctx, size[0], size[1], size[2], color)
            }
            GeometryType::Cylinder { radius, height } => {
                Mesh::cylinder(ctx, *radius, *height, 16, color)
            }
            GeometryType::Sphere { radius } => Mesh::sphere(ctx, *radius, 16, 12, color),
            GeometryType::Capsule { radius, length } => {
                // Approximate capsule as cylinder
                Mesh::cylinder(ctx, *radius, *length, 16, color)
            }
        }
    }

    /// Create a box mesh.
    fn create_box_mesh(
        ctx: &WgpuContext,
        width: f32,
        height: f32,
        depth: f32,
        color: [f32; 3],
    ) -> Mesh {
        let hw = width / 2.0;
        let hh = height / 2.0;
        let hd = depth / 2.0;

        let mut vertices = Vec::with_capacity(24);

        // Front face (+Z)
        let normal = [0.0, 0.0, 1.0];
        vertices.push(Vertex {
            position: [-hw, -hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, -hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, hh, hd],
            normal,
            color,
        });

        // Back face (-Z)
        let normal = [0.0, 0.0, -1.0];
        vertices.push(Vertex {
            position: [hw, -hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, -hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, hh, -hd],
            normal,
            color,
        });

        // Top face (+Y)
        let normal = [0.0, 1.0, 0.0];
        vertices.push(Vertex {
            position: [-hw, hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, hh, -hd],
            normal,
            color,
        });

        // Bottom face (-Y)
        let normal = [0.0, -1.0, 0.0];
        vertices.push(Vertex {
            position: [-hw, -hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, -hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, -hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, -hh, hd],
            normal,
            color,
        });

        // Right face (+X)
        let normal = [1.0, 0.0, 0.0];
        vertices.push(Vertex {
            position: [hw, -hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, -hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [hw, hh, hd],
            normal,
            color,
        });

        // Left face (-X)
        let normal = [-1.0, 0.0, 0.0];
        vertices.push(Vertex {
            position: [-hw, -hh, -hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, -hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, hh, hd],
            normal,
            color,
        });
        vertices.push(Vertex {
            position: [-hw, hh, -hd],
            normal,
            color,
        });

        let mut indices = Vec::with_capacity(36);
        for face in 0..6_u32 {
            let base = face * 4;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        Mesh::new(ctx, &vertices, Some(&indices), Some("box"))
    }

    /// Update joint angles and recompute transforms.
    ///
    /// # Arguments
    /// * `left_angles` - Joint angles for left arm (8 joints).
    /// * `right_angles` - Joint angles for right arm (8 joints).
    pub fn update_joints(&mut self, left_angles: &[f32], right_angles: &[f32]) {
        self.link_transforms.clear();
        self.link_transforms
            .insert("base_link".to_owned(), Mat4::IDENTITY);

        // Build transform tree
        for joint in &self.joints {
            let parent_transform = self
                .link_transforms
                .get(&joint.parent_link)
                .copied()
                .unwrap_or(Mat4::IDENTITY);

            // Determine joint angle
            let angle = Self::get_joint_angle(&joint.name, left_angles, right_angles);

            // Compute joint rotation
            let rotation = Mat4::from_quat(Quat::from_axis_angle(joint.axis, angle));

            // Compute child transform
            let child_transform = parent_transform * joint.origin * rotation;

            self.link_transforms
                .insert(joint.child_link.clone(), child_transform);
        }

        // Update link world transforms
        for link in &mut self.links {
            let link_transform = self
                .link_transforms
                .get(&link.name)
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            link.world_transform = link_transform * link.local_transform;
        }
    }

    fn get_joint_angle(joint_name: &str, left_angles: &[f32], right_angles: &[f32]) -> f32 {
        // Parse joint name to extract index
        // Expected format: left_joint_1, right_joint_1, etc.
        if let Some(suffix) = joint_name.strip_prefix("left_joint_") {
            if let Ok(idx) = suffix.parse::<usize>() {
                if idx > 0 && idx <= left_angles.len() {
                    return left_angles[idx - 1];
                }
            }
        }
        if let Some(suffix) = joint_name.strip_prefix("right_joint_") {
            if let Ok(idx) = suffix.parse::<usize>() {
                if idx > 0 && idx <= right_angles.len() {
                    return right_angles[idx - 1];
                }
            }
        }
        0.0
    }

    /// Render the robot model.
    pub fn render(
        &self,
        ctx: &WgpuContext,
        viewer: &dyn Viewer,
        lights: &[&dyn Light],
        render_pass: &mut wgpu::RenderPass<'_>,
    ) {
        // Render grid first
        self.grid_material.update_camera(ctx, viewer);
        render_pass.set_pipeline(self.grid_material.pipeline());
        render_pass.set_bind_group(0, self.grid_material.camera_bind_group(), &[]);
        render_pass.set_vertex_buffer(0, self.grid_mesh.vertex_buffer().slice());
        if let Some(index_buffer) = self.grid_mesh.index_buffer() {
            render_pass.set_index_buffer(index_buffer.slice(), index_buffer.format());
            render_pass.draw_indexed(0..self.grid_mesh.draw_count(), 0, 0..1);
        }

        // Render robot links
        for link in &self.links {
            self.material
                .update_uniforms(ctx, viewer, link.world_transform, lights);

            render_pass.set_pipeline(self.material.pipeline());
            render_pass.set_bind_group(0, self.material.camera_bind_group(), &[]);
            render_pass.set_bind_group(1, self.material.model_bind_group(), &[]);
            render_pass.set_vertex_buffer(0, link.mesh.vertex_buffer().slice());

            if let Some(index_buffer) = link.mesh.index_buffer() {
                render_pass.set_index_buffer(index_buffer.slice(), index_buffer.format());
                render_pass.draw_indexed(0..link.mesh.draw_count(), 0, 0..1);
            } else {
                render_pass.draw(0..link.mesh.draw_count(), 0..1);
            }
        }
    }

    /// Spawn the robot as ECS entities with parent-child hierarchy.
    ///
    /// Consumes the `RobotModel` and moves each link's mesh into the ECS world.
    /// Each link becomes an entity with Transform, GlobalTransform, and MeshRenderer.
    /// The hierarchy follows the URDF joint tree.
    /// Returns the root entity.
    #[cfg(feature = "ecs")]
    pub fn spawn_into_world(
        self,
        world: &mut hecs::World,
        _format: wgpu::TextureFormat,
    ) -> anyhow::Result<hecs::Entity> {
        use crate::ecs::components::rendering::{
            FrustumCullable, MaterialHandle, MeshHandle, MeshRenderer, Visible,
        };
        use crate::ecs::components::transform::{Children, GlobalTransform, Parent, Transform};

        let material_arc: Arc<dyn crate::ecs::bridge::MaterialResource> = Arc::new(self.material);

        let mut link_entities: HashMap<String, hecs::Entity> = HashMap::new();

        for link in self.links {
            let transform = Transform::from_matrix(link.local_transform);
            let global = GlobalTransform(link.world_transform);

            let renderer = MeshRenderer {
                mesh: MeshHandle(Arc::new(link.mesh)),
                material: MaterialHandle(Arc::clone(&material_arc)),
                visible: true,
                cast_shadow: true,
                receive_shadow: true,
            };

            let entity = world.spawn((transform, global, renderer, FrustumCullable, Visible));
            link_entities.insert(link.name, entity);
        }

        // Build parent-child relationships from joints
        let mut children_map: HashMap<hecs::Entity, Vec<hecs::Entity>> = HashMap::new();

        for joint in &self.joints {
            if let (Some(&parent_entity), Some(&child_entity)) = (
                link_entities.get(&joint.parent_link),
                link_entities.get(&joint.child_link),
            ) {
                world.insert_one(child_entity, Parent(parent_entity)).ok();
                children_map
                    .entry(parent_entity)
                    .or_default()
                    .push(child_entity);
            }
        }

        for (parent_entity, children) in children_map {
            world.insert_one(parent_entity, Children(children)).ok();
        }

        // Find root: base_link or first link without a Parent
        let root = link_entities
            .get("base_link")
            .copied()
            .or_else(|| {
                link_entities
                    .values()
                    .find(|&&entity| world.get::<&Parent>(entity).is_err())
                    .copied()
            })
            .unwrap_or_else(|| world.spawn((Transform::identity(), GlobalTransform::default())));

        Ok(root)
    }

    /// Get the bounding box of the robot.
    pub fn aabb(&self) -> Aabb {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for link in &self.links {
            let link_aabb = link.mesh.aabb();
            // Transform corners to world space
            let corners = [
                link.world_transform.transform_point3(link_aabb.min),
                link.world_transform.transform_point3(link_aabb.max),
            ];
            for corner in corners {
                min = min.min(corner);
                max = max.max(corner);
            }
        }

        Aabb::new(min, max)
    }
}
