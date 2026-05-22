//! ECS render system.
//!
//! Extracts rendering data from the ECS World and issues draw calls.

use crate::context::WgpuContext;
use crate::ecs::components::rendering::{CameraComponent, LightComponent, MeshRenderer, Visible};
use crate::ecs::components::transform::GlobalTransform;
use crate::renderer::light::{Light, LightType, LightUniforms};
use crate::renderer::viewer::Camera;
use glam::Vec3;
use std::sync::Arc;

/// A lightweight light wrapper for ECS LightComponent data.
///
/// Implements the `Light` trait so it can be passed to material uniform updates.
struct EcsLight {
    light_type: LightType,
    color: Vec3,
    intensity: f32,
    position_or_direction: Vec3,
}

impl Light for EcsLight {
    fn light_type(&self) -> LightType {
        self.light_type
    }

    fn uniforms(&self) -> LightUniforms {
        let w = match self.light_type {
            LightType::Directional | LightType::Ambient => 0.0,
            LightType::Point | LightType::Spot => 1.0,
        };
        LightUniforms {
            direction_or_position: [
                self.position_or_direction.x,
                self.position_or_direction.y,
                self.position_or_direction.z,
                w,
            ],
            color_intensity: [self.color.x, self.color.y, self.color.z, self.intensity],
            attenuation: [1.0, 0.09, 0.032, 0.0],
        }
    }
}

/// Find the active camera entity and return a cloned Camera.
///
/// Returns `None` if no active camera exists.
fn find_active_camera(world: &hecs::World) -> Option<Camera> {
    for (_, (cam, _global)) in &mut world.query::<(&CameraComponent, &GlobalTransform)>() {
        if cam.active {
            return Some(cam.camera.clone());
        }
    }
    None
}

/// Collect all lights from the ECS World.
fn collect_lights(world: &hecs::World) -> Vec<EcsLight> {
    let mut lights = Vec::new();
    for (_, (light, global)) in &mut world.query::<(&LightComponent, &GlobalTransform)>() {
        let position_or_direction = match light.light_type {
            LightType::Directional => {
                // Extract the forward direction (-Z axis) from the transform matrix.
                global.0.transform_vector3(-Vec3::Z).normalize()
            }
            _ => {
                // Extract position from the transform.
                global.0.transform_point3(Vec3::ZERO)
            }
        };

        lights.push(EcsLight {
            light_type: light.light_type,
            color: light.color,
            intensity: light.intensity,
            position_or_direction,
        });
    }
    lights
}

/// Pre-collected draw command with owned Arc handles.
struct DrawCommand {
    material: Arc<dyn crate::ecs::bridge::MaterialResource>,
    mesh: Arc<dyn crate::renderer::geometry::Geometry + Send + Sync>,
    global_transform: glam::Mat4,
}

/// ECS render system.
///
/// Queries the World for the active camera, lights, and visible MeshRenderer
/// entities, updates material uniforms, then issues draw calls.
///
/// # Rendering approach
///
/// Material uniform updates (camera, model, lights) are performed first using
/// the collected data. Then draw calls are issued using the Arc-shared GPU
/// resources, which remain valid as long as the Arcs are alive.
pub fn render_system(
    world: &hecs::World,
    ctx: &WgpuContext,
    render_pass: &mut wgpu::RenderPass<'_>,
) {
    // 1. Find active camera.
    let Some(camera) = find_active_camera(world) else {
        return;
    };

    // 2. Collect lights.
    let ecs_lights = collect_lights(world);
    let light_refs: Vec<&dyn Light> = ecs_lights.iter().map(|l| -> &dyn Light { l }).collect();

    // 3. Collect drawable entities with Arc clones.
    let mut draw_commands: Vec<DrawCommand> = Vec::new();
    {
        let mut query = world
            .query::<(&MeshRenderer, &GlobalTransform)>()
            .with::<&Visible>();
        for (_, (renderer, global)) in &mut query {
            if !renderer.visible {
                continue;
            }
            draw_commands.push(DrawCommand {
                material: Arc::clone(&renderer.material.0),
                mesh: Arc::clone(&renderer.mesh.0),
                global_transform: global.0,
            });
        }
    }

    // 4. Update uniforms and issue draw calls.
    for cmd in &draw_commands {
        // Update material uniforms with camera, model matrix, and lights.
        cmd.material
            .update_uniforms(ctx, &camera, cmd.global_transform, &light_refs);

        // Set pipeline and bind groups.
        render_pass.set_pipeline(cmd.material.pipeline());
        render_pass.set_bind_group(0, cmd.material.camera_bind_group(), &[]);
        render_pass.set_bind_group(1, cmd.material.model_bind_group(), &[]);

        // Bind any additional material-specific bind groups (e.g. group 2+)
        for (group, bind_group) in cmd.material.extra_bind_groups() {
            render_pass.set_bind_group(group, bind_group, &[]);
        }

        // Draw the mesh manually (avoiding the Geometry::draw lifetime issue).
        render_pass.set_vertex_buffer(0, cmd.mesh.vertex_buffer().slice());
        if let Some(index_buffer) = cmd.mesh.index_buffer() {
            render_pass.set_index_buffer(index_buffer.slice(), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..cmd.mesh.draw_count(), 0, 0..1);
        } else {
            render_pass.draw(0..cmd.mesh.draw_count(), 0..1);
        }
    }
}
