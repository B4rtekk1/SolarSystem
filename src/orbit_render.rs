use crate::camera::Camera;
use crate::color::Color;
use crate::constants::{DEFAULT_CAMERA_DISTANCE, MAX_ORBIT_WIDTH_SCALE, MIN_ORBIT_WIDTH_SCALE, MOON_ORBIT_FORECAST_MAX_POINTS, MOON_ORBIT_FORECAST_SAMPLE_YEARS,
                       MOON_ORBIT_HALF_WIDTH_PIXELS, ORBIT_FORECAST_MAX_POINTS, ORBIT_FORECAST_SAMPLE_YEARS, ORBIT_TRAIL_POINTS, ORBIT_VERTICES_PER_SEGMENT,
                       OrbitSegment, PLANET_ORBIT_HALF_WIDTH_PIXELS};
use crate::ecs::{CelestialKind, Entity, World};
use crate::nbody::NBodySimulation;
use crate::uniforms::*;
use glam::{DVec3, Vec3};
use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;

struct OrbitForecastRequest {
    physics: NBodySimulation,
}

pub struct OrbitForecastResult {
    pub orbit_forecasts: Vec<Vec<DVec3>>,
    pub moon_orbit_offsets: Vec<(Entity, Vec<DVec3>)>,
}

pub struct OrbitForecastWorker {
    request_tx: mpsc::Sender<OrbitForecastRequest>,
    result_rx: mpsc::Receiver<OrbitForecastResult>,
    request_in_flight: bool,
}

impl OrbitForecastWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<OrbitForecastRequest>();
        let (result_tx, result_rx) = mpsc::channel::<OrbitForecastResult>();

        thread::Builder::new()
            .name("orbit-forecast".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let orbit_forecasts = request.physics.forecast_full_planet_orbits(
                        ORBIT_FORECAST_MAX_POINTS,
                        ORBIT_FORECAST_SAMPLE_YEARS,
                    );
                    let moon_orbit_offsets = request.physics.forecast_full_moon_orbit_offsets(
                        MOON_ORBIT_FORECAST_MAX_POINTS,
                        MOON_ORBIT_FORECAST_SAMPLE_YEARS,
                    );

                    if result_tx
                        .send(OrbitForecastResult {
                            orbit_forecasts,
                            moon_orbit_offsets,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to spawn orbit forecast worker");

        Self {
            request_tx,
            result_rx,
            request_in_flight: false,
        }
    }

    pub fn request(&mut self, physics: &NBodySimulation) -> bool {
        if self.request_in_flight {
            return false;
        }

        match self.request_tx.send(OrbitForecastRequest {
            physics: physics.clone(),
        }) {
            Ok(()) => {
                self.request_in_flight = true;
                true
            }
            Err(_) => false,
        }
    }

    pub fn poll(&mut self) -> Option<OrbitForecastResult> {
        let mut latest = None;

        loop {
            match self.result_rx.try_recv() {
                Ok(result) => {
                    latest = Some(result);
                    self.request_in_flight = false;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.request_in_flight = false;
                    break;
                }
            }
        }

        latest
    }
}

pub fn build_orbit_segments(
    trails: &[VecDeque<Vec3>],
    forecasts: &[Vec<DVec3>],
    moon_offsets: &[(Entity, Vec<DVec3>)],
    world: &World,
    physics: &NBodySimulation,
    planet_entities: &[Entity],
    orbit_width_scale: f32,
    segments: &mut Vec<OrbitSegment>,
) {
    let planet_half_width_pixels = PLANET_ORBIT_HALF_WIDTH_PIXELS * orbit_width_scale;
    let moon_half_width_pixels = MOON_ORBIT_HALF_WIDTH_PIXELS * orbit_width_scale;

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
        let parent_position = dvec3_to_vec3(physics.position(parent));
        let moon_position = dvec3_to_vec3(physics.position(*moon));
        let current_offset = moon_position - parent_position;
        let start_index = nearest_orbit_offset_index(offsets, current_offset);
        let remaining_segments = offsets.len().saturating_sub(start_index + 1);
        if remaining_segments == 0 {
            continue;
        }

        let mut previous = moon_position;

        for (segment_index, offset) in offsets.iter().skip(start_index + 1).enumerate() {
            let age = (segment_index + 1) as f32 / remaining_segments as f32;
            let alpha = 0.36 * (1.0 - age).max(0.0) + 0.05;
            let vertex_color = [future_color[0], future_color[1], future_color[2], alpha];
            let current = parent_position + dvec3_to_vec3(*offset);
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
    let trail_segments = ORBIT_TRAIL_POINTS.saturating_sub(1);
    let forecast_segments = ORBIT_FORECAST_MAX_POINTS;
    let planet_segments = (trail_segments + forecast_segments) * planet_count;
    let moon_segments = world.count_kind(CelestialKind::Moon) * MOON_ORBIT_FORECAST_MAX_POINTS;
    (planet_segments + moon_segments).max(1)
}

pub fn orbit_draw_vertex_count(segments: &[OrbitSegment]) -> u32 {
    (segments.len() * ORBIT_VERTICES_PER_SEGMENT) as u32
}

pub fn create_orbit_trails(physics: &NBodySimulation) -> Vec<VecDeque<Vec3>> {
    physics
        .planet_entities()
        .iter()
        .map(|entity| {
            let mut trail = VecDeque::with_capacity(ORBIT_TRAIL_POINTS);
            trail.push_back(dvec3_to_vec3(physics.position(*entity)));
            trail
        })
        .collect()
}

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

        let score = current_direction.dot(offset / offset_length);
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
            &mut segments,
        );

        assert_eq!(segments.len(), 2);
        assert_vec3_near(
            segment_end(segments.last().unwrap()),
            parent_position + offsets[2],
        );
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
