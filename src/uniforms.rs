use crate::constants::SELECTION_FOCUS_BRIGHTNESS;
use crate::ecs::{CelestialKind, Entity, MaterialComponent, World};
use crate::nbody::NBodySimulation;
use glam::{DVec3, Mat4, Vec3};
use std::f32::consts::TAU;

pub type ObjectUniform = [f32; 32];

const MOON_ORBIT_VISUAL_PADDING: f32 = 0.08;
const MOON_ORBIT_VISUAL_SCALE: f32 = 8.0;
const MOON_SIBLING_VISUAL_PADDING: f32 = 0.025;

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

pub fn rendered_entity_position(world: &World, physics: &NBodySimulation, entity: Entity) -> Vec3 {
    let position = dvec3_to_vec3(physics.render_position(entity));
    if world.kind(entity) != CelestialKind::Moon {
        return position;
    }

    let Some(parent) = world.parent(entity).map(|parent| parent.entity) else {
        return position;
    };

    let parent_position = dvec3_to_vec3(physics.render_position(parent));
    let offset = dvec3_to_vec3(physics.render_position(entity) - physics.render_position(parent));
    parent_position + rendered_moon_offset(world, entity, offset)
}

pub fn rendered_moon_offset(world: &World, moon: Entity, offset: Vec3) -> Vec3 {
    let Some(parent) = world.parent(moon).map(|parent| parent.entity) else {
        return offset;
    };

    let offset_length = offset.length();
    if offset_length <= f32::EPSILON {
        return Vec3::X * visible_moon_orbit_radius(world, parent, moon, 0.0);
    }

    offset / offset_length * visible_moon_orbit_radius(world, parent, moon, offset_length)
}

fn visible_moon_orbit_radius(
    world: &World,
    parent: Entity,
    moon: Entity,
    physical_offset: f32,
) -> f32 {
    let parent_radius = world.body(parent).render_radius;
    let moon_radius = world.body(moon).render_radius;
    let clearance = parent_radius + moon_radius + MOON_ORBIT_VISUAL_PADDING;
    let parent_clearance = if physical_offset >= clearance {
        physical_offset
    } else {
        clearance + physical_offset * MOON_ORBIT_VISUAL_SCALE
    };
    parent_clearance.max(minimum_sibling_shell_radius(world, parent, moon))
}

fn minimum_sibling_shell_radius(world: &World, parent: Entity, moon: Entity) -> f32 {
    let parent_radius = world.body(parent).render_radius;
    let moon_body = world.body(moon);
    let moon_orbit_radius = moon_body
        .orbit
        .map_or(f32::INFINITY, |orbit| orbit.semi_major_axis.abs());
    let mut previous_outer_edge = parent_radius + MOON_ORBIT_VISUAL_PADDING;

    let mut siblings = world
        .children_of_kind(parent, CelestialKind::Moon)
        .map(|sibling| {
            let body = world.body(sibling);
            let orbit_radius = body
                .orbit
                .map_or(f32::INFINITY, |orbit| orbit.semi_major_axis.abs());
            (sibling, orbit_radius, body.render_radius)
        })
        .collect::<Vec<_>>();
    siblings.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.0.index().cmp(&b.0.index()))
    });

    for (sibling, sibling_orbit_radius, sibling_radius) in siblings {
        let shell_radius = previous_outer_edge + sibling_radius;
        if sibling == moon {
            return shell_radius;
        }

        if sibling_orbit_radius <= moon_orbit_radius {
            previous_outer_edge = shell_radius + sibling_radius + MOON_SIBLING_VISUAL_PADDING;
        }
    }

    parent_radius + moon_body.render_radius + MOON_ORBIT_VISUAL_PADDING
}

fn selection_emphasis(world: &World, entity: Entity, selected_body: Option<Entity>) -> f32 {
    let Some(selected_body) = selected_body else {
        return 0.0;
    };

    if is_selection_focus(world, entity, selected_body) {
        1.0
    } else {
        0.0
    }
}

fn is_selection_focus(world: &World, entity: Entity, selected_body: Entity) -> bool {
    entity == selected_body
        || (world.kind(selected_body) == CelestialKind::Planet
            && world.kind(entity) == CelestialKind::Moon
            && world
                .parent(entity)
                .is_some_and(|parent| parent.entity == selected_body))
}

pub fn entity_object_uniform(
    world: &World,
    physics: &NBodySimulation,
    entity: Entity,
    shader_time: f32,
    selected_body: Option<Entity>,
) -> ObjectUniform {
    let body = world.body(entity);
    let rotation = world.rotation(entity);
    let render = world.render(entity);
    let position = rendered_entity_position(world, physics, entity);
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
                material.brightness,
                0.0,
                selection_brightness(world, entity, selected_body),
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
            let selection_emphasis = selection_emphasis(world, entity, selected_body);
            let selection_brightness = selection_brightness(world, entity, selected_body);
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

pub fn selection_brightness(world: &World, entity: Entity, selected_body: Option<Entity>) -> f32 {
    let Some(selected_body) = selected_body else {
        return 1.0;
    };

    if is_selection_focus(world, entity, selected_body) {
        SELECTION_FOCUS_BRIGHTNESS
    } else {
        1.0
    }
}

pub fn ray_sphere_distance(
    origin: Vec3,
    direction: Vec3,
    center: Vec3,
    radius: f32,
) -> Option<f32> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        color::Color,
        ecs::{
            BodyComponent, MaterialComponent, ObjectBundle, RenderComponent, RotationComponent,
            StarMaterial, SurfaceMaterial,
        },
        nbody::NBodyConfig,
        orbit::Orbit,
    };

    #[test]
    fn rendered_moon_position_is_outside_parent_disc() {
        let mut world = World::default();
        let star = world.spawn(ObjectBundle {
            name: "Star".to_string(),
            kind: CelestialKind::Star,
            parent: None,
            body: BodyComponent::new(1.989e30, 696_340.0, None),
            rotation: RotationComponent { speed: 0.0 },
            render: RenderComponent {
                material: MaterialComponent::Star(StarMaterial {
                    base_color: Color::rgb(1.0, 0.8, 0.2),
                    accent_color: Color::rgb(1.0, 1.0, 0.5),
                    brightness: 1.0,
                    surface_temperature: 5778.0,
                }),
            },
            atmosphere: None,
            ring: None,
        });
        let planet = world.spawn(test_surface_body(
            "Planet",
            CelestialKind::Planet,
            Some(star),
            5.972e24,
            6_371.0,
            Orbit::circular(1.0, 1.0),
        ));
        let moon = world.spawn(test_surface_body(
            "Moon",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            1_737.4,
            Orbit::circular(0.00257, 12.0),
        ));
        let physics = NBodySimulation::from_world(&world, NBodyConfig::default());

        let parent_position = rendered_entity_position(&world, &physics, planet);
        let moon_position = rendered_entity_position(&world, &physics, moon);
        let visible_distance = (moon_position - parent_position).length();
        let minimum_separation = world.body(planet).render_radius + world.body(moon).render_radius;

        assert!(visible_distance > minimum_separation);
    }

    #[test]
    fn close_moon_orbits_get_separate_visible_shells() {
        let mut world = World::default();
        let star = world.spawn(ObjectBundle {
            name: "Star".to_string(),
            kind: CelestialKind::Star,
            parent: None,
            body: BodyComponent::new(1.989e30, 696_340.0, None),
            rotation: RotationComponent { speed: 0.0 },
            render: RenderComponent {
                material: MaterialComponent::Star(StarMaterial {
                    base_color: Color::rgb(1.0, 0.8, 0.2),
                    accent_color: Color::rgb(1.0, 1.0, 0.5),
                    brightness: 1.0,
                    surface_temperature: 5778.0,
                }),
            },
            atmosphere: None,
            ring: None,
        });
        let planet = world.spawn(test_surface_body(
            "Planet",
            CelestialKind::Planet,
            Some(star),
            5.972e24,
            6_371.0,
            Orbit::circular(1.0, 1.0),
        ));
        let inner = world.spawn(test_surface_body(
            "Inner",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            1_737.4,
            Orbit::circular(0.00257, 12.0),
        ));
        let outer = world.spawn(test_surface_body(
            "Outer",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            1_737.4,
            Orbit::circular(0.00258, 12.0),
        ));

        let inner_shell = minimum_sibling_shell_radius(&world, planet, inner);
        let outer_shell = minimum_sibling_shell_radius(&world, planet, outer);
        let minimum_gap = world.body(inner).render_radius + world.body(outer).render_radius;

        assert!(outer_shell - inner_shell > minimum_gap);
    }

    fn test_surface_body(
        name: &str,
        kind: CelestialKind,
        parent: Option<Entity>,
        mass: f32,
        radius: f32,
        orbit: Orbit,
    ) -> ObjectBundle {
        ObjectBundle {
            name: name.to_string(),
            kind,
            parent,
            body: BodyComponent::new(mass, radius, Some(orbit)),
            rotation: RotationComponent { speed: 0.0 },
            render: RenderComponent {
                material: MaterialComponent::Surface(SurfaceMaterial {
                    base_color: Color::rgb(0.5, 0.5, 0.5),
                    accent_color: Color::rgb(0.7, 0.7, 0.7),
                    roughness: 0.8,
                    metallic: 0.0,
                }),
            },
            atmosphere: None,
            ring: None,
        }
    }
}
