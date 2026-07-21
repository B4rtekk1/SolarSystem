use crate::camera::Camera;
use crate::color::Color;
use crate::constants::{
    DEFAULT_CAMERA_DISTANCE, KEPLER_MOON_ORBIT_SEGMENTS, KEPLER_PLANET_ORBIT_SEGMENTS,
    MAX_ORBIT_WIDTH_SCALE, MIN_ORBIT_WIDTH_SCALE, MOON_ORBIT_HALF_WIDTH_PIXELS,
    ORBIT_VERTICES_PER_SEGMENT, OrbitSegment, PLANET_ORBIT_HALF_WIDTH_PIXELS,
};
use crate::ecs::{CelestialKind, Entity, World};
use crate::nbody::NBodySimulation;
use crate::uniforms::*;
use glam::{DVec3, Vec3};
use std::collections::VecDeque;

pub fn build_orbit_segments(
    trails: &[VecDeque<Vec3>],
    forecasts: &[Vec<DVec3>],
    moon_offsets: &[(Entity, Vec<DVec3>)],
    world: &World,
    physics: &NBodySimulation,
    planet_entities: &[Entity],
    orbit_width_scale: f32,
    show_planet_orbits: bool,
    show_moon_orbits: bool,
    orbit_thickness_scale: f32,
    segments: &mut Vec<OrbitSegment>,
) {
    let moon_shell_radii = moon_visual_shell_radii(world);
    let width_scale = orbit_width_scale * orbit_thickness_scale.max(0.0);
    let planet_half_width_pixels = PLANET_ORBIT_HALF_WIDTH_PIXELS * width_scale;
    let moon_half_width_pixels = MOON_ORBIT_HALF_WIDTH_PIXELS * width_scale;

    if show_planet_orbits {
        for (trail, entity) in trails.iter().zip(planet_entities.iter()) {
            let segment_count = trail.len().saturating_sub(1);
            if segment_count == 0 {
                continue;
            }

            let color = orbit_color(world, *entity);
            let mut previous = trail[0];
            for (segment_index, current) in trail.iter().skip(1).enumerate() {
                let age = (segment_index + 1) as f32 / segment_count as f32;
                let vertex_color = [color[0], color[1], color[2], 0.08 + age * 0.34];
                segments.push(orbit_segment(
                    previous,
                    *current,
                    vertex_color,
                    planet_half_width_pixels,
                ));
                previous = *current;
            }
        }

        for ((forecast, entity), trail) in forecasts
            .iter()
            .zip(planet_entities.iter())
            .zip(trails.iter())
        {
            if forecast.len() < 2 {
                continue;
            }

            let color = orbit_color(world, *entity);
            let future_color = [
                (color[0] * 1.35).min(1.0),
                (color[1] * 1.35).min(1.0),
                (color[2] * 1.35).min(1.0),
            ];
            let segment_count = forecast.len() - 1;
            let mut previous = trail
                .back()
                .copied()
                .unwrap_or_else(|| dvec3_to_vec3(forecast[0]));

            for (segment_index, current) in forecast.iter().skip(1).enumerate() {
                let age = (segment_index + 1) as f32 / segment_count as f32;
                let alpha = 0.48 * (1.0 - age).max(0.0) + 0.06;
                let vertex_color = [future_color[0], future_color[1], future_color[2], alpha];
                let current = dvec3_to_vec3(*current);
                segments.push(orbit_segment(
                    previous,
                    current,
                    vertex_color,
                    planet_half_width_pixels,
                ));
                previous = current;
            }
        }
    }

    if !show_moon_orbits {
        return;
    }

    for (moon, offsets) in moon_offsets {
        let Some(parent) = world.parent(*moon).map(|parent| parent.entity) else {
            continue;
        };
        if offsets.len() < 2 {
            continue;
        }

        let color = orbit_color(world, *moon);
        let future_color = [
            (color[0] * 1.45).min(1.0),
            (color[1] * 1.45).min(1.0),
            (color[2] * 1.45).min(1.0),
        ];
        let parent_position = rendered_entity_position(world, physics, parent);

        // FIX #1: Use raw_current_offset (before rendered_moon_offset transform) for
        // nearest_orbit_offset_index, so it compares in the same space as the raw offsets
        // stored in the forecast buffer.
        let raw_current_offset =
            dvec3_to_vec3(physics.render_position(*moon) - physics.render_position(parent));
        let current_offset =
            rendered_moon_offset_cached(world, *moon, raw_current_offset, &moon_shell_radii);
        let moon_position = parent_position + current_offset;

        let start_index = nearest_orbit_offset_index(offsets, raw_current_offset);

        let segment_count = offsets.len() - 1;
        let mut previous = moon_position;

        for segment_index in 0..segment_count {
            let offset_index = (start_index + segment_index + 1) % offsets.len();
            let offset = offsets[offset_index];
            let age = (segment_index + 1) as f32 / segment_count as f32;
            let alpha = 0.36 * (1.0 - age).max(0.0) + 0.05;
            let vertex_color = [future_color[0], future_color[1], future_color[2], alpha];
            let current = parent_position
                + rendered_moon_offset_cached(
                    world,
                    *moon,
                    dvec3_to_vec3(offset),
                    &moon_shell_radii,
                );
            if !is_reasonable_moon_orbit_segment(previous, current, parent_position) {
                previous = current;
                continue;
            }
            segments.push(orbit_segment(
                previous,
                current,
                vertex_color,
                moon_half_width_pixels,
            ));
            previous = current;
        }
    }
}

pub fn build_kepler_orbit_segments(
    world: &World,
    physics: &NBodySimulation,
    planet_entities: &[Entity],
    orbit_width_scale: f32,
    show_planet_orbits: bool,
    show_moon_orbits: bool,
    orbit_thickness_scale: f32,
    segments: &mut Vec<OrbitSegment>,
) {
    let moon_shell_radii = moon_visual_shell_radii(world);
    let width_scale = orbit_width_scale * orbit_thickness_scale.max(0.0);
    let planet_half_width_pixels = PLANET_ORBIT_HALF_WIDTH_PIXELS * width_scale;
    let moon_half_width_pixels = MOON_ORBIT_HALF_WIDTH_PIXELS * width_scale;

    if show_planet_orbits {
        for entity in planet_entities {
            append_kepler_orbit(
                world,
                physics,
                *entity,
                KEPLER_PLANET_ORBIT_SEGMENTS,
                planet_half_width_pixels,
                &moon_shell_radii,
                segments,
            );
        }
    }

    if show_moon_orbits {
        for moon in world.entities_of_kind(CelestialKind::Moon) {
            append_kepler_orbit(
                world,
                physics,
                moon,
                KEPLER_MOON_ORBIT_SEGMENTS,
                moon_half_width_pixels,
                &moon_shell_radii,
                segments,
            );
        }
    }
}

fn append_kepler_orbit(
    world: &World,
    physics: &NBodySimulation,
    entity: Entity,
    segment_count: usize,
    half_width_pixels: f32,
    moon_shell_radii: &[Option<f32>],
    segments: &mut Vec<OrbitSegment>,
) {
    let Some(orbit) = world.body(entity).orbit else {
        return;
    };
    let parent_position = world
        .parent(entity)
        .map(|parent| rendered_entity_position(world, physics, parent.entity))
        .unwrap_or(Vec3::ZERO);
    let color = orbit_color(world, entity);
    let color = [
        (color[0] * 1.25).min(1.0),
        (color[1] * 1.25).min(1.0),
        (color[2] * 1.25).min(1.0),
        if world.kind(entity) == CelestialKind::Moon {
            0.25
        } else {
            0.42
        },
    ];

    let mut previous =
        kepler_orbit_point(world, entity, orbit, parent_position, 0.0, moon_shell_radii);
    for index in 1..=segment_count {
        let anomaly = std::f32::consts::TAU * index as f32 / segment_count as f32;
        let current = kepler_orbit_point(
            world,
            entity,
            orbit,
            parent_position,
            anomaly,
            moon_shell_radii,
        );
        segments.push(orbit_segment(previous, current, color, half_width_pixels));
        previous = current;
    }
}

fn kepler_orbit_point(
    world: &World,
    entity: Entity,
    orbit: crate::orbit::Orbit,
    parent_position: Vec3,
    true_anomaly: f32,
    moon_shell_radii: &[Option<f32>],
) -> Vec3 {
    let semi_major = orbit.semi_major_axis.abs().max(f32::EPSILON);
    let semi_minor = orbit
        .semi_minor_axis
        .abs()
        .min(semi_major)
        .max(f32::EPSILON);
    let eccentricity = (1.0 - (semi_minor / semi_major).powi(2)).max(0.0).sqrt();
    let semi_latus_rectum = semi_major * (1.0 - eccentricity * eccentricity);
    let radius = semi_latus_rectum / (1.0 + eccentricity * true_anomaly.cos()).max(f32::EPSILON);
    let (sin_anomaly, cos_anomaly) = true_anomaly.sin_cos();
    let (sin_periapsis, cos_periapsis) = orbit.argument_of_periapsis.sin_cos();
    let periapsis_x = radius * cos_anomaly * cos_periapsis - radius * sin_anomaly * sin_periapsis;
    let periapsis_z = radius * cos_anomaly * sin_periapsis + radius * sin_anomaly * cos_periapsis;
    let (sin_inclination, cos_inclination) = orbit.inclination.sin_cos();
    let inclined_x = periapsis_x;
    let inclined_y = -periapsis_z * sin_inclination;
    let inclined_z = periapsis_z * cos_inclination;
    let (sin_node, cos_node) = orbit.ascending_node.sin_cos();
    let raw_offset = Vec3::new(
        orbit.center[0] + inclined_x * cos_node + inclined_z * sin_node,
        orbit.center[1] + inclined_y,
        orbit.center[2] - inclined_x * sin_node + inclined_z * cos_node,
    );

    if world.kind(entity) == CelestialKind::Moon {
        parent_position + rendered_moon_offset_cached(world, entity, raw_offset, moon_shell_radii)
    } else {
        parent_position + raw_offset
    }
}

fn moon_visual_shell_radii(world: &World) -> Vec<Option<f32>> {
    let mut radii = vec![None; world.entity_capacity()];
    for moon in world.entities_of_kind(CelestialKind::Moon) {
        let Some(parent) = world.parent(moon).map(|parent| parent.entity) else {
            continue;
        };
        radii[moon.index()] = Some(moon_visual_shell_radius(world, parent, moon));
    }
    radii
}

fn rendered_moon_offset_cached(
    world: &World,
    moon: Entity,
    offset: Vec3,
    moon_shell_radii: &[Option<f32>],
) -> Vec3 {
    let Some(shell_radius) = moon_shell_radii
        .get(moon.index())
        .and_then(|radius| *radius)
    else {
        return rendered_moon_offset(world, moon, offset);
    };

    rendered_moon_offset_with_shell(world, moon, offset, shell_radius)
}

fn is_reasonable_moon_orbit_segment(previous: Vec3, current: Vec3, parent_position: Vec3) -> bool {
    if !previous.is_finite() || !current.is_finite() || !parent_position.is_finite() {
        return false;
    }

    let previous_radius = (previous - parent_position).length();
    let current_radius = (current - parent_position).length();
    let orbit_radius = previous_radius.max(current_radius);
    if orbit_radius <= f32::EPSILON {
        return false;
    }

    (current - previous).length() <= orbit_radius * 0.35
}

pub fn orbit_segment(
    start: Vec3,
    end: Vec3,
    color: [f32; 4],
    half_width_pixels: f32,
) -> OrbitSegment {
    [
        start.x,
        start.y,
        start.z,
        1.0,
        end.x,
        end.y,
        end.z,
        1.0,
        color[0],
        color[1],
        color[2],
        color[3],
        half_width_pixels,
        0.0,
        0.0,
        0.0,
    ]
}

pub fn orbit_width_scale(camera: &Camera) -> f32 {
    (DEFAULT_CAMERA_DISTANCE / camera.distance())
        .clamp(MIN_ORBIT_WIDTH_SCALE, MAX_ORBIT_WIDTH_SCALE)
}

pub fn orbit_color(world: &World, entity: Entity) -> [f32; 3] {
    world
        .surface_material(entity)
        .map_or(Color::rgb(0.7, 0.7, 0.7), |material| material.base_color)
        .as_array()
}

pub fn max_orbit_segment_count(world: &World, planet_count: usize) -> usize {
    let planet_segments = KEPLER_PLANET_ORBIT_SEGMENTS * planet_count;
    let moon_segments = world.count_kind(CelestialKind::Moon) * KEPLER_MOON_ORBIT_SEGMENTS;
    (planet_segments + moon_segments).max(1)
}

pub fn orbit_draw_vertex_count(segments: &[OrbitSegment]) -> u32 {
    (segments.len() * ORBIT_VERTICES_PER_SEGMENT) as u32
}

/// Finds the index in `offsets` whose direction best matches `current_offset`,
/// combining angular similarity with radial proximity to avoid false matches
/// on elliptical orbits where two points can share a similar angle.
///
/// NOTE: `current_offset` and `offsets` must be in the same coordinate space
/// (both raw, before `rendered_moon_offset`).
pub fn nearest_orbit_offset_index(offsets: &[DVec3], current_offset: Vec3) -> usize {
    if offsets.is_empty() {
        return 0;
    }

    let current_length = current_offset.length();
    if current_length <= f32::EPSILON {
        return 0;
    }

    let current_direction = current_offset / current_length;
    let mut best_index = 0;
    let mut best_score = f32::NEG_INFINITY;

    for (index, offset) in offsets.iter().enumerate() {
        let offset = dvec3_to_vec3(*offset);
        let offset_length = offset.length();
        if offset_length <= f32::EPSILON {
            continue;
        }

        // FIX #2: Weight angular similarity by radial proximity so that on
        // elliptical orbits two points with a similar direction but very
        // different radii no longer score equally.
        let angular_score = current_direction.dot(offset / offset_length);
        let radial_ratio = (current_length / offset_length).min(offset_length / current_length);
        let score = angular_score * radial_ratio;

        if score > best_score {
            best_index = index;
            best_score = score;
        }
    }

    best_index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        color::Color,
        ecs::{
            BodyComponent, CelestialKind, MaterialComponent, ObjectBundle, RenderComponent,
            RotationComponent, StarMaterial, SurfaceMaterial, World,
        },
        nbody::NBodyConfig,
        orbit::Orbit,
    };

    #[test]
    fn moon_forecast_segments_do_not_wrap_to_the_start() {
        let mut world = World::default();
        let star = world.spawn(ObjectBundle {
            name: "Star".to_string(),
            kind: CelestialKind::Star,
            parent: None,
            body: BodyComponent::new(1.989e30, 1.0, None),
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
            0.1,
            Orbit::circular(1.0, 1.0),
        ));
        let moon = world.spawn(test_surface_body(
            "Moon",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            0.03,
            Orbit::circular(0.3, 1.0),
        ));
        let physics = NBodySimulation::from_world(&world, NBodyConfig::default());
        let parent_position = physics.position(planet);
        let start_offset = physics.position(moon) - parent_position;
        let offsets = vec![
            start_offset,
            start_offset + DVec3::new(0.0, 0.08, 0.03),
            start_offset + DVec3::new(0.0, 0.16, 0.12),
        ];
        let moon_offsets = vec![(moon, offsets.clone())];
        let mut segments = Vec::new();

        build_orbit_segments(
            &[],
            &[],
            &moon_offsets,
            &world,
            &physics,
            &[],
            1.0,
            true,
            true,
            1.0,
            &mut segments,
        );

        assert_eq!(segments.len(), 2);
        assert_vec3_near(
            segment_end(segments.last().unwrap()),
            parent_position + offsets[2],
        );
    }

    #[test]
    fn moon_forecast_skips_stale_long_segments() {
        let mut world = World::default();
        let star = world.spawn(test_star());
        let planet = world.spawn(test_surface_body(
            "Planet",
            CelestialKind::Planet,
            Some(star),
            5.972e24,
            0.1,
            Orbit::circular(1.0, 1.0),
        ));
        let moon = world.spawn(test_surface_body(
            "Moon",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            0.03,
            Orbit::circular(0.3, 1.0),
        ));
        let physics = NBodySimulation::from_world(&world, NBodyConfig::default());
        let offsets = vec![
            DVec3::new(0.3, 0.0, 0.0),
            DVec3::new(-0.3, 0.0, 0.0),
            DVec3::new(-0.29, 0.01, 0.0),
        ];
        let moon_offsets = vec![(moon, offsets)];
        let mut segments = Vec::new();

        build_orbit_segments(
            &[],
            &[],
            &moon_offsets,
            &world,
            &physics,
            &[],
            1.0,
            true,
            true,
            1.0,
            &mut segments,
        );

        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn moon_forecast_wraps_after_last_matching_point() {
        let mut world = World::default();
        let star = world.spawn(test_star());
        let planet = world.spawn(test_surface_body(
            "Planet",
            CelestialKind::Planet,
            Some(star),
            5.972e24,
            0.1,
            Orbit::circular(1.0, 1.0),
        ));
        let moon = world.spawn(test_surface_body(
            "Moon",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            0.03,
            Orbit::circular(0.3, 1.0),
        ));
        let physics = NBodySimulation::from_world(&world, NBodyConfig::default());
        let current_offset = physics.position(moon) - physics.position(planet);
        let offsets = vec![
            current_offset + DVec3::new(0.0, 0.01, 0.0),
            current_offset + DVec3::new(0.0, 0.02, 0.0),
            current_offset,
        ];
        let moon_offsets = vec![(moon, offsets)];
        let mut segments = Vec::new();

        build_orbit_segments(
            &[],
            &[],
            &moon_offsets,
            &world,
            &physics,
            &[],
            1.0,
            true,
            true,
            1.0,
            &mut segments,
        );

        assert_eq!(segments.len(), 2);
    }

    fn test_star() -> ObjectBundle {
        ObjectBundle {
            name: "Star".to_string(),
            kind: CelestialKind::Star,
            parent: None,
            body: BodyComponent::new(1.989e30, 1.0, None),
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
        }
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

    fn segment_end(segment: &OrbitSegment) -> DVec3 {
        DVec3::new(segment[4] as f64, segment[5] as f64, segment[6] as f64)
    }

    fn assert_vec3_near(actual: DVec3, expected: DVec3) {
        let error = (actual - expected).length();
        assert!(error < 1.0e-6, "expected {expected:?}, got {actual:?}");
    }
}
