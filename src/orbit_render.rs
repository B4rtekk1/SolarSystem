use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;
use glam::{DVec3, Vec3};
use crate::ecs::{CelestialKind, Entity, World};
use crate::nbody::NBodySimulation;
use crate::{dvec3_to_vec3, OrbitSegment, DEFAULT_CAMERA_DISTANCE, MAX_ORBIT_WIDTH_SCALE, MIN_ORBIT_WIDTH_SCALE, MOON_ORBIT_FORECAST_MAX_POINTS, MOON_ORBIT_FORECAST_SAMPLE_YEARS, MOON_ORBIT_HALF_WIDTH_PIXELS, ORBIT_FORECAST_MAX_POINTS, ORBIT_FORECAST_SAMPLE_YEARS, ORBIT_TRAIL_POINTS, ORBIT_VERTICES_PER_SEGMENT, PLANET_ORBIT_HALF_WIDTH_PIXELS};
use crate::camera::Camera;
use crate::color::Color;

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
        let ring_len = offsets.len().saturating_sub(1);
        if ring_len < 2 {
            continue;
        };

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
        let mut previous = moon_position;

        for segment_index in 1..=ring_len {
            let age = segment_index as f32 / ring_len as f32;
            let alpha = 0.36 * (1.0 - age).max(0.0) + 0.05;
            let vertex_color = [future_color[0], future_color[1], future_color[2], alpha];
            let offset_index = (start_index + segment_index) % ring_len;
            let current = parent_position + dvec3_to_vec3(offsets[offset_index]);
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

pub fn orbit_segment(start: Vec3, end: Vec3, color: [f32; 4], half_width_pixels: f32) -> OrbitSegment {
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
    let ring_len = offsets.len().saturating_sub(1);
    if ring_len == 0 {
        return 0;
    }

    let current_length = current_offset.length();
    if current_length <= f32::EPSILON {
        return 0;
    }

    let current_direction = current_offset / current_length;
    let mut best_index = 0;
    let mut best_score = f32::NEG_INFINITY;

    for (index, offset) in offsets.iter().take(ring_len).enumerate() {
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