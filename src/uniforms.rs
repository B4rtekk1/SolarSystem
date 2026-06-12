use std::f32::consts::TAU;
use glam::{DVec3, Mat4, Vec3};
use crate::ecs::{CelestialKind, Entity, MaterialComponent, World};
use crate::nbody::NBodySimulation;
use crate::constants::SELECTION_FOCUS_BRIGHTNESS;

pub type ObjectUniform = [f32; 32];

fn object_uniform(
    model: Mat4,
    base_color: [f32; 3],
    accent_color: [f32; 3],
    emissive: [f32; 4],
    shader_params: [f32; 4],
) -> ObjectUniform {
    let mut uniform = [0.0; 32];
    uniform[..16].copy_from_slice(&model.to_cols_array());
    uniform[16..20].copy_from_slice(&[base_color[0], base_color[1], base_color[2], 1.0]);
    uniform[20..24].copy_from_slice(&[accent_color[0], accent_color[1], accent_color[2], 1.0]);
    uniform[24..28].copy_from_slice(&emissive);
    uniform[28..32].copy_from_slice(&shader_params);
    uniform
}

pub fn dvec3_to_vec3(position: DVec3) -> Vec3 {
    Vec3::new(position.x as f32, position.y as f32, position.z as f32)
}

fn selection_emphasis(world: &World, entity: Entity, selected_planet: Option<Entity>) -> f32 {
    let Some(selected_planet) = selected_planet else {
        return 0.0;
    };

    if is_selection_focus(world, entity, selected_planet) {
        1.0
    } else {
        0.0
    }
}

fn is_selection_focus(world: &World, entity: Entity, selected_planet: Entity) -> bool {
    entity == selected_planet
        || (world.kind(entity) == CelestialKind::Moon
        && world
        .parent(entity)
        .is_some_and(|parent| parent.entity == selected_planet))
}

pub fn entity_object_uniform(
    world: &World,
    physics: &NBodySimulation,
    entity: Entity,
    shader_time: f32,
    selected_planet: Option<Entity>,
) -> ObjectUniform {
    let body = world.body(entity);
    let rotation = world.rotation(entity);
    let render = world.render(entity);
    let position = dvec3_to_vec3(physics.position(entity));
    let rotation_angle = (shader_time * rotation.speed).rem_euclid(TAU);
    let model = Mat4::from_translation(position)
        * Mat4::from_rotation_y(rotation_angle)
        * Mat4::from_scale(Vec3::splat(body.render_radius));

    match render.material {
        MaterialComponent::Star(material) => object_uniform(
            model,
            material.base_color.as_array(),
            material.accent_color.as_array(),
            [
                material.brightness * selection_brightness(world, entity, selected_planet),
                0.0,
                0.0,
                0.0,
            ],
            [
                ((material.surface_temperature - 2500.0) / 9500.0).clamp(0.0, 1.0),
                1.35,
                18.0,
                shader_time,
            ],
        ),
        MaterialComponent::Surface(material) => {
            let atmosphere = world.atmosphere(entity);
            let accent_color =
                atmosphere.map_or(material.accent_color, |atmosphere| atmosphere.color);
            let atmosphere_density = atmosphere.map_or(0.0, |atmosphere| {
                atmosphere.density * atmosphere.radius_multiplier.max(0.0)
            });
            let selection_emphasis = selection_emphasis(world, entity, selected_planet);
            let selection_brightness = selection_brightness(world, entity, selected_planet);
            object_uniform(
                model,
                material.base_color.as_array(),
                accent_color.as_array(),
                [0.0, selection_emphasis, selection_brightness, 0.0],
                [
                    material.roughness,
                    material.metallic,
                    atmosphere_density,
                    shader_time,
                ],
            )
        }
    }
}

pub fn selection_brightness(world: &World, entity: Entity, selected_planet: Option<Entity>) -> f32 {
    let Some(selected_planet) = selected_planet else {
        return 1.0;
    };

    if is_selection_focus(world, entity, selected_planet) {
        SELECTION_FOCUS_BRIGHTNESS
    } else {
        1.0
    }
}

pub fn ray_sphere_distance(origin: Vec3, direction: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let offset = origin - center;
    let b = offset.dot(direction);
    let c = offset.length_squared() - radius * radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }

    let root = discriminant.sqrt();
    let near = -b - root;
    if near >= 0.0 {
        return Some(near);
    }

    let far = -b + root;
    if far >= 0.0 { Some(far) } else { None }
}
