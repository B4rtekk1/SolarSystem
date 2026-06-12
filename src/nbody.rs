use crate::{
    ecs::{CelestialKind, Entity, World},
    orbit::Orbit,
};
use glam::DVec3;
use std::f64::consts::TAU;

const GRAVITATIONAL_CONSTANT_M3_KG_S2: f64 = 6.674_30e-11;
const SOLAR_MASS_KG: f64 = 1.988_47e30;
const ASTRONOMICAL_UNIT_METERS: f64 = 149_597_870_700.0;
const JULIAN_YEAR_SECONDS: f64 = 31_557_600.0;
const GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2: f64 =
    GRAVITATIONAL_CONSTANT_M3_KG_S2 * SOLAR_MASS_KG * JULIAN_YEAR_SECONDS * JULIAN_YEAR_SECONDS
        / (ASTRONOMICAL_UNIT_METERS * ASTRONOMICAL_UNIT_METERS * ASTRONOMICAL_UNIT_METERS);
const MAX_FRAME_SECONDS: f64 = 0.1;
const MAX_STEPS_PER_FRAME: usize = 96;

#[derive(Debug, Clone, Copy)]
pub struct NBodyConfig {
    pub years_per_second: f64,
    pub fixed_step_years: f64,
    pub softening_length: f64,
}

impl Default for NBodyConfig {
    fn default() -> Self {
        Self {
            years_per_second: 0.22,
            fixed_step_years: 1.0 / 720.0,
            softening_length: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Body {
    mass: f64,
    position: DVec3,
    velocity: DVec3,
}

#[derive(Debug, Clone, Copy)]
struct OrbitForecastTracker {
    normal: DVec3,
    previous_direction: DVec3,
    accumulated_angle: f64,
    complete: bool,
}

#[derive(Debug, Clone, Copy)]
struct MoonOrbitTarget {
    entity: Entity,
    parent: Entity,
}

#[derive(Debug, Clone)]
pub struct NBodySimulation {
    bodies: Vec<Body>,
    body_index_by_entity: Vec<Option<usize>>,
    planet_entities: Vec<Entity>,
    moon_orbits: Vec<MoonOrbitTarget>,
    current_accelerations: Vec<DVec3>,
    next_accelerations: Vec<DVec3>,
    config: NBodyConfig,
    accumulator_years: f64,
    elapsed_years: f64,
}

impl NBodySimulation {
    pub fn from_world(world: &World, config: NBodyConfig) -> Self {
        let mut body_index_by_entity = vec![None; world.entity_capacity()];
        let mut bodies = Vec::with_capacity(world.entity_capacity());
        let mut moon_orbits = Vec::new();
        let star_entity = world.first_entity_of_kind(CelestialKind::Star);
        let star_mass = star_entity.map_or(1.0, |entity| {
            let body = world.body(entity);
            push_body(
                &mut bodies,
                &mut body_index_by_entity,
                entity,
                body.mass,
                body.orbit
                    .map_or(DVec3::ZERO, |orbit| initial_orbit_state(&orbit, 0.0, 0.0).0),
                DVec3::ZERO,
            )
        });

        let planet_entities: Vec<Entity> = world.entities_of_kind(CelestialKind::Planet).collect();
        for planet_entity in &planet_entities {
            let planet_body = world.body(*planet_entity);
            let planet_mass = kg_to_solar_masses(planet_body.mass as f64);
            let moon_entities: Vec<Entity> = world
                .children_of_kind(*planet_entity, CelestialKind::Moon)
                .collect();
            let moon_mass_sum = moon_entities
                .iter()
                .map(|entity| kg_to_solar_masses(world.body(*entity).mass as f64))
                .sum::<f64>();
            let (position, velocity) = planet_body
                .orbit
                .map_or((DVec3::ZERO, DVec3::ZERO), |orbit| {
                    initial_orbit_state(&orbit, star_mass, planet_mass + moon_mass_sum)
                });
            let planet_body_index = bodies.len();
            push_body(
                &mut bodies,
                &mut body_index_by_entity,
                *planet_entity,
                planet_body.mass,
                position,
                velocity,
            );

            let mut parent_velocity_offset = DVec3::ZERO;
            for moon_entity in &moon_entities {
                let moon_body = world.body(*moon_entity);
                let moon_mass = kg_to_solar_masses(moon_body.mass as f64);
                let (moon_offset, moon_relative_velocity) =
                    moon_body.orbit.map_or((DVec3::ZERO, DVec3::ZERO), |orbit| {
                        initial_orbit_state(&orbit, planet_mass, moon_mass)
                    });

                if planet_mass > f64::EPSILON {
                    parent_velocity_offset -= moon_relative_velocity * (moon_mass / planet_mass);
                }

                push_body(
                    &mut bodies,
                    &mut body_index_by_entity,
                    *moon_entity,
                    moon_body.mass,
                    position + moon_offset,
                    velocity + moon_relative_velocity,
                );
            }

            if let Some(parent_body) = bodies.get_mut(planet_body_index) {
                parent_body.velocity += parent_velocity_offset;
            }

            moon_orbits.extend(moon_entities.into_iter().map(|entity| MoonOrbitTarget {
                entity,
                parent: *planet_entity,
            }));
        }

        let system_momentum = bodies.iter().skip(1).fold(DVec3::ZERO, |momentum, body| {
            momentum + body.velocity * body.mass
        });
        if let Some(star_body) = bodies.first_mut() {
            if star_body.mass > f64::EPSILON {
                star_body.velocity = -system_momentum / star_body.mass;
            }
        }

        let acceleration_buffer_len = bodies.len();

        Self {
            bodies,
            body_index_by_entity,
            planet_entities,
            moon_orbits,
            current_accelerations: vec![DVec3::ZERO; acceleration_buffer_len],
            next_accelerations: vec![DVec3::ZERO; acceleration_buffer_len],
            config,
            accumulator_years: 0.0,
            elapsed_years: 0.0,
        }
    }

    pub fn advance_scaled(&mut self, real_seconds: f64, time_scale: f64) {
        if !real_seconds.is_finite()
            || real_seconds <= 0.0
            || !time_scale.is_finite()
            || time_scale <= 0.0
        {
            return;
        }

        self.accumulator_years +=
            real_seconds.min(MAX_FRAME_SECONDS) * self.config.years_per_second * time_scale;

        let mut steps = 0;
        while self.accumulator_years >= self.config.fixed_step_years && steps < MAX_STEPS_PER_FRAME
        {
            self.step(self.config.fixed_step_years);
            self.accumulator_years -= self.config.fixed_step_years;
            steps += 1;
        }

        if steps == MAX_STEPS_PER_FRAME {
            self.accumulator_years = 0.0;
        }
    }

    #[cfg(test)]
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    pub fn planet_entities(&self) -> &[Entity] {
        &self.planet_entities
    }

    pub fn position(&self, entity: Entity) -> DVec3 {
        self.body_index_by_entity
            .get(entity.index())
            .and_then(|body_index| *body_index)
            .and_then(|body_index| self.bodies.get(body_index))
            .map_or(DVec3::ZERO, |body| body.position)
    }

    pub fn render_position(&self, entity: Entity) -> DVec3 {
        let extrapolation_years = if self.accumulator_years.is_finite()
            && self.config.fixed_step_years.is_finite()
            && self.config.fixed_step_years > 0.0
        {
            self.accumulator_years
                .clamp(0.0, self.config.fixed_step_years)
        } else {
            0.0
        };

        self.body_index_by_entity
            .get(entity.index())
            .and_then(|body_index| *body_index)
            .and_then(|body_index| self.bodies.get(body_index))
            .map_or(DVec3::ZERO, |body| {
                body.position + body.velocity * extrapolation_years
            })
    }

    pub fn sun_position(&self) -> DVec3 {
        self.bodies
            .first()
            .map_or(DVec3::ZERO, |body| body.position)
    }

    pub fn planet_position(&self, planet_index: usize) -> DVec3 {
        self.planet_entities
            .get(planet_index)
            .map_or(DVec3::ZERO, |entity| self.position(*entity))
    }

    pub fn elapsed_years(&self) -> f64 {
        self.elapsed_years
    }

    pub fn forecast_full_planet_orbits(
        &self,
        max_sample_count: usize,
        sample_interval_years: f64,
    ) -> Vec<Vec<DVec3>> {
        let mut forecast =
            vec![Vec::with_capacity(max_sample_count + 1); self.planet_entities.len()];
        for (index, path) in forecast.iter_mut().enumerate() {
            path.push(self.planet_position(index));
        }

        if max_sample_count == 0
            || !sample_interval_years.is_finite()
            || sample_interval_years <= 0.0
        {
            return forecast;
        }

        let mut simulation = self.clone();
        simulation.accumulator_years = 0.0;
        let mut trackers = self.create_forecast_trackers();

        for _ in 0..max_sample_count {
            simulation.advance_years(sample_interval_years);
            let sun_position = simulation.sun_position();

            for (index, path) in forecast.iter_mut().enumerate() {
                let Some(tracker) = trackers.get_mut(index) else {
                    continue;
                };
                if tracker.complete {
                    continue;
                }

                let planet_position = simulation.planet_position(index);
                let relative_position = planet_position - sun_position;
                let direction = projected_direction(
                    relative_position,
                    tracker.normal,
                    tracker.previous_direction,
                );
                let delta_angle =
                    signed_angle(tracker.previous_direction, direction, tracker.normal);

                tracker.accumulated_angle += delta_angle.max(0.0);
                tracker.previous_direction = direction;
                path.push(planet_position);

                if tracker.accumulated_angle >= TAU {
                    tracker.complete = true;
                }
            }

            if trackers.iter().all(|tracker| tracker.complete) {
                break;
            }
        }

        forecast
    }

    pub fn forecast_full_moon_orbits(
        &self,
        max_sample_count: usize,
        sample_interval_years: f64,
    ) -> Vec<(Entity, Vec<DVec3>)> {
        let mut forecast = self
            .moon_orbits
            .iter()
            .map(|target| {
                let mut path = Vec::with_capacity(max_sample_count + 1);
                path.push(self.position(target.entity));
                (target.entity, path)
            })
            .collect::<Vec<_>>();

        if max_sample_count == 0
            || !sample_interval_years.is_finite()
            || sample_interval_years <= 0.0
        {
            return forecast;
        }

        let mut simulation = self.clone();
        simulation.accumulator_years = 0.0;
        let mut trackers = self.create_moon_forecast_trackers();

        for _ in 0..max_sample_count {
            simulation.advance_years(sample_interval_years);

            for ((target, (_, path)), tracker) in self
                .moon_orbits
                .iter()
                .zip(forecast.iter_mut())
                .zip(trackers.iter_mut())
            {
                if tracker.complete {
                    continue;
                }

                let moon_position = simulation.position(target.entity);
                let relative_position = moon_position - simulation.position(target.parent);
                let direction = projected_direction(
                    relative_position,
                    tracker.normal,
                    tracker.previous_direction,
                );
                let delta_angle =
                    signed_angle(tracker.previous_direction, direction, tracker.normal);

                tracker.accumulated_angle += delta_angle.max(0.0);
                tracker.previous_direction = direction;
                path.push(moon_position);

                if tracker.accumulated_angle >= TAU {
                    tracker.complete = true;
                }
            }

            if trackers.iter().all(|tracker| tracker.complete) {
                break;
            }
        }

        forecast
    }

    pub fn forecast_full_moon_orbit_offsets(
        &self,
        max_sample_count: usize,
        sample_interval_years: f64,
    ) -> Vec<(Entity, Vec<DVec3>)> {
        let mut forecast = self
            .moon_orbits
            .iter()
            .map(|target| {
                let mut path = Vec::with_capacity(max_sample_count + 1);
                path.push(self.position(target.entity) - self.position(target.parent));
                (target.entity, path)
            })
            .collect::<Vec<_>>();

        if max_sample_count == 0
            || !sample_interval_years.is_finite()
            || sample_interval_years <= 0.0
        {
            return forecast;
        }

        let mut simulation = self.clone();
        simulation.accumulator_years = 0.0;
        let mut trackers = self.create_moon_forecast_trackers();

        for _ in 0..max_sample_count {
            simulation.advance_years(sample_interval_years);

            for ((target, (_, path)), tracker) in self
                .moon_orbits
                .iter()
                .zip(forecast.iter_mut())
                .zip(trackers.iter_mut())
            {
                if tracker.complete {
                    continue;
                }

                let relative_position =
                    simulation.position(target.entity) - simulation.position(target.parent);
                let direction = projected_direction(
                    relative_position,
                    tracker.normal,
                    tracker.previous_direction,
                );
                let delta_angle =
                    signed_angle(tracker.previous_direction, direction, tracker.normal);

                tracker.accumulated_angle += delta_angle.max(0.0);
                tracker.previous_direction = direction;
                path.push(relative_position);

                if tracker.accumulated_angle >= TAU {
                    tracker.complete = true;
                }
            }

            if trackers.iter().all(|tracker| tracker.complete) {
                break;
            }
        }

        forecast
    }

    fn step(&mut self, dt: f64) {
        self.ensure_acceleration_buffers();
        write_accelerations(
            &self.bodies,
            self.config.softening_length,
            &mut self.current_accelerations,
        );
        let half_dt_squared = 0.5 * dt * dt;

        for (body, acceleration) in self
            .bodies
            .iter_mut()
            .zip(self.current_accelerations.iter())
        {
            body.position += body.velocity * dt + *acceleration * half_dt_squared;
        }

        write_accelerations(
            &self.bodies,
            self.config.softening_length,
            &mut self.next_accelerations,
        );
        for ((body, current), next) in self
            .bodies
            .iter_mut()
            .zip(self.current_accelerations.iter())
            .zip(self.next_accelerations.iter())
        {
            body.velocity += (*current + *next) * (0.5 * dt);
        }

        self.elapsed_years += dt;
    }

    fn advance_years(&mut self, years: f64) {
        if !years.is_finite() || years <= 0.0 {
            return;
        }

        let mut remaining = years;
        while remaining > self.config.fixed_step_years {
            self.step(self.config.fixed_step_years);
            remaining -= self.config.fixed_step_years;
        }

        if remaining > 0.0 {
            self.step(remaining);
        }
    }

    fn create_forecast_trackers(&self) -> Vec<OrbitForecastTracker> {
        let Some(sun) = self.bodies.first() else {
            return Vec::new();
        };

        self.planet_entities
            .iter()
            .filter_map(|entity| {
                self.body_index_by_entity
                    .get(entity.index())
                    .and_then(|body_index| *body_index)
                    .and_then(|body_index| self.bodies.get(body_index))
            })
            .map(|body| {
                let relative_position = body.position - sun.position;
                let relative_velocity = body.velocity - sun.velocity;
                let normal =
                    normalized_or_fallback(relative_position.cross(relative_velocity), DVec3::Y);
                let direction = projected_direction(relative_position, normal, DVec3::X);

                OrbitForecastTracker {
                    normal,
                    previous_direction: direction,
                    accumulated_angle: 0.0,
                    complete: false,
                }
            })
            .collect()
    }

    fn create_moon_forecast_trackers(&self) -> Vec<OrbitForecastTracker> {
        self.moon_orbits
            .iter()
            .map(|target| {
                let relative_position = self.position(target.entity) - self.position(target.parent);
                let relative_velocity = self.velocity(target.entity) - self.velocity(target.parent);
                let normal =
                    normalized_or_fallback(relative_position.cross(relative_velocity), DVec3::Y);
                let direction = projected_direction(relative_position, normal, DVec3::X);

                OrbitForecastTracker {
                    normal,
                    previous_direction: direction,
                    accumulated_angle: 0.0,
                    complete: false,
                }
            })
            .collect()
    }

    pub fn velocity(&self, entity: Entity) -> DVec3 {
        self.body_index_by_entity
            .get(entity.index())
            .and_then(|body_index| *body_index)
            .and_then(|body_index| self.bodies.get(body_index))
            .map_or(DVec3::ZERO, |body| body.velocity)
    }

    fn ensure_acceleration_buffers(&mut self) {
        let body_count = self.bodies.len();
        if self.current_accelerations.len() != body_count {
            self.current_accelerations.resize(body_count, DVec3::ZERO);
        }
        if self.next_accelerations.len() != body_count {
            self.next_accelerations.resize(body_count, DVec3::ZERO);
        }
    }
}

fn write_accelerations(bodies: &[Body], softening_length: f64, accelerations: &mut [DVec3]) {
    accelerations.fill(DVec3::ZERO);
    let softening_squared = if softening_length.is_finite() && softening_length > 0.0 {
        softening_length * softening_length
    } else {
        0.0
    };

    for i in 0..bodies.len() {
        for j in (i + 1)..bodies.len() {
            let delta = bodies[j].position - bodies[i].position;
            let distance_squared = delta.length_squared() + softening_squared;
            if distance_squared <= f64::EPSILON {
                continue;
            }
            let inverse_distance = distance_squared.sqrt().recip();
            let inverse_distance_cubed = inverse_distance * inverse_distance * inverse_distance;
            let acceleration_base =
                GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2 * delta * inverse_distance_cubed;

            accelerations[i] += acceleration_base * bodies[j].mass;
            accelerations[j] -= acceleration_base * bodies[i].mass;
        }
    }
}

fn push_body(
    bodies: &mut Vec<Body>,
    body_index_by_entity: &mut [Option<usize>],
    entity: Entity,
    mass_kg: f32,
    position: DVec3,
    velocity: DVec3,
) -> f64 {
    let mass = kg_to_solar_masses(mass_kg as f64);
    let body_index = bodies.len();
    bodies.push(Body {
        mass,
        position,
        velocity,
    });
    if let Some(slot) = body_index_by_entity.get_mut(entity.index()) {
        *slot = Some(body_index);
    }
    mass
}

fn kg_to_solar_masses(kg: f64) -> f64 {
    kg / SOLAR_MASS_KG
}

fn initial_orbit_state(orbit: &Orbit, central_mass: f64, body_mass: f64) -> (DVec3, DVec3) {
    let semi_major_axis = (orbit.semi_major_axis as f64).abs().max(1.0e-9);
    let semi_minor_axis = (orbit.semi_minor_axis as f64)
        .abs()
        .clamp(1.0e-9, semi_major_axis);
    let eccentricity_squared = (1.0
        - semi_minor_axis * semi_minor_axis / (semi_major_axis * semi_major_axis))
        .clamp(0.0, 1.0);
    let eccentricity = eccentricity_squared.sqrt();
    let semi_latus_rectum = semi_major_axis * (1.0 - eccentricity_squared);
    let true_anomaly = orbit.phase as f64;
    let (sin_anomaly, cos_anomaly) = true_anomaly.sin_cos();
    let radius = semi_latus_rectum / (1.0 + eccentricity * cos_anomaly).max(1.0e-9);
    let position = orbit_position_from_plane(orbit, radius * cos_anomaly, radius * sin_anomaly);

    let standard_gravitational_parameter =
        GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2 * (central_mass + body_mass);
    let velocity = if standard_gravitational_parameter > 0.0 && semi_latus_rectum > 1.0e-12 {
        let speed_scale = (standard_gravitational_parameter / semi_latus_rectum).sqrt();
        let radial_speed = speed_scale * eccentricity * sin_anomaly;
        let tangential_speed = speed_scale * (1.0 + eccentricity * cos_anomaly);
        let velocity_x = radial_speed * cos_anomaly - tangential_speed * sin_anomaly;
        let velocity_z = radial_speed * sin_anomaly + tangential_speed * cos_anomaly;
        let orbit_direction = if orbit.angular_speed < 0.0 { -1.0 } else { 1.0 };

        orbit_vector_from_plane(orbit, velocity_x, velocity_z) * orbit_direction
    } else {
        DVec3::ZERO
    };

    (position, velocity)
}

fn orbit_position_from_plane(orbit: &Orbit, x: f64, z: f64) -> DVec3 {
    DVec3::new(
        orbit.center[0] as f64,
        orbit.center[1] as f64,
        orbit.center[2] as f64,
    ) + orbit_vector_from_plane(orbit, x, z)
}

fn orbit_vector_from_plane(orbit: &Orbit, x: f64, z: f64) -> DVec3 {
    let (sin_inclination, cos_inclination) = (orbit.inclination as f64).sin_cos();
    DVec3::new(x, -z * sin_inclination, z * cos_inclination)
}

fn projected_direction(vector: DVec3, normal: DVec3, fallback: DVec3) -> DVec3 {
    let projected = vector - normal * vector.dot(normal);
    normalized_or_fallback(projected, fallback)
}

fn signed_angle(from: DVec3, to: DVec3, normal: DVec3) -> f64 {
    let sin = normal.dot(from.cross(to));
    let cos = from.dot(to).clamp(-1.0, 1.0);
    sin.atan2(cos)
}

fn normalized_or_fallback(vector: DVec3, fallback: DVec3) -> DVec3 {
    let length = vector.length();
    if length > f64::EPSILON {
        vector / length
    } else {
        fallback
    }
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
    };

    #[test]
    fn step_keeps_total_momentum_stable() {
        let config = NBodyConfig {
            years_per_second: 1.0,
            fixed_step_years: 0.01,
            softening_length: 0.0,
        };
        let mut simulation = NBodySimulation {
            bodies: vec![
                Body {
                    mass: 1.0,
                    position: DVec3::ZERO,
                    velocity: DVec3::new(0.0, -0.001, 0.0),
                },
                Body {
                    mass: 0.001,
                    position: DVec3::X,
                    velocity: DVec3::Y,
                },
            ],
            body_index_by_entity: Vec::new(),
            planet_entities: Vec::new(),
            moon_orbits: Vec::new(),
            current_accelerations: vec![DVec3::ZERO; 2],
            next_accelerations: vec![DVec3::ZERO; 2],
            config,
            accumulator_years: 0.0,
            elapsed_years: 0.0,
        };
        let initial_momentum = total_momentum(&simulation.bodies);

        simulation.step(config.fixed_step_years);

        let momentum_error = (total_momentum(&simulation.bodies) - initial_momentum).length();
        assert!(momentum_error < 1.0e-12);
    }

    #[test]
    fn default_config_uses_unsoftened_gravity() {
        assert_eq!(NBodyConfig::default().softening_length, 0.0);
    }

    #[test]
    fn acceleration_uses_newton_inverse_square_law() {
        let bodies = vec![
            Body {
                mass: 1.0,
                position: DVec3::ZERO,
                velocity: DVec3::ZERO,
            },
            Body {
                mass: 0.001,
                position: DVec3::X,
                velocity: DVec3::ZERO,
            },
        ];
        let mut accelerations = vec![DVec3::ZERO; 2];

        write_accelerations(&bodies, 0.0, &mut accelerations);

        assert!(
            (accelerations[0].x - GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2 * 0.001).abs()
                < 1.0e-12
        );
        assert!(accelerations[0].y.abs() < 1.0e-12);
        assert!(accelerations[0].z.abs() < 1.0e-12);
        assert!((accelerations[1].x + GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2).abs() < 1.0e-12);
    }

    #[test]
    fn elliptical_initial_state_places_central_mass_at_focus() {
        let orbit = Orbit::elliptical(2.0, 3.0_f32.sqrt(), 1.0);

        let (position, velocity) = initial_orbit_state(&orbit, 1.0, 0.0);

        assert!((position.x - 1.0).abs() < 1.0e-6);
        assert!(position.y.abs() < 1.0e-12);
        assert!(position.z.abs() < 1.0e-12);
        assert!(velocity.x.abs() < 1.0e-12);
        assert!(
            (velocity.z - (GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2 * 1.5).sqrt()).abs()
                < 1.0e-6
        );
    }

    #[test]
    fn render_position_uses_unstepped_accumulator() {
        let entity = Entity::from_index(0);
        let mut simulation = NBodySimulation {
            bodies: vec![Body {
                mass: 1.0,
                position: DVec3::ZERO,
                velocity: DVec3::new(2.0, 0.0, 0.0),
            }],
            body_index_by_entity: vec![Some(0)],
            planet_entities: vec![entity],
            moon_orbits: Vec::new(),
            current_accelerations: vec![DVec3::ZERO],
            next_accelerations: vec![DVec3::ZERO],
            config: NBodyConfig {
                years_per_second: 1.0,
                fixed_step_years: 1.0,
                softening_length: 0.0,
            },
            accumulator_years: 0.0,
            elapsed_years: 0.0,
        };

        simulation.advance_scaled(0.05, 1.0);

        assert_eq!(simulation.position(entity), DVec3::ZERO);
        assert!((simulation.render_position(entity).x - 0.1).abs() < 1.0e-12);
    }

    #[test]
    fn forecast_stops_after_full_orbit() {
        let planet = Entity::from_index(1);
        let simulation = NBodySimulation {
            bodies: vec![
                Body {
                    mass: 1.0,
                    position: DVec3::ZERO,
                    velocity: DVec3::ZERO,
                },
                Body {
                    mass: 0.0,
                    position: DVec3::X,
                    velocity: DVec3::Y * TAU,
                },
            ],
            body_index_by_entity: vec![Some(0), Some(1)],
            planet_entities: vec![planet],
            moon_orbits: Vec::new(),
            current_accelerations: vec![DVec3::ZERO; 2],
            next_accelerations: vec![DVec3::ZERO; 2],
            config: NBodyConfig {
                years_per_second: 1.0,
                fixed_step_years: 1.0 / 2048.0,
                softening_length: 0.0,
            },
            accumulator_years: 0.0,
            elapsed_years: 0.0,
        };

        let forecast = simulation.forecast_full_planet_orbits(256, 1.0 / 64.0);

        assert_eq!(forecast.len(), 1);
        assert!(forecast[0].len() > 50);
        assert!(forecast[0].len() < 80);
    }

    #[test]
    fn from_world_adds_moons_as_bodies() {
        let mut world = World::default();
        let star = world.spawn(test_star());
        let planet = world.spawn(test_surface_body(
            "Parent",
            CelestialKind::Planet,
            Some(star),
            5.972e24,
            0.1,
            Orbit::circular(1.0, 1.0),
        ));
        let moon = world.spawn(test_surface_body(
            "Child",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            0.03,
            Orbit::circular(0.1, 1.0),
        ));

        let simulation = NBodySimulation::from_world(
            &world,
            NBodyConfig {
                years_per_second: 1.0,
                fixed_step_years: 0.01,
                softening_length: 0.0,
            },
        );

        assert_eq!(simulation.body_count(), 3);
        assert_eq!(simulation.planet_entities(), &[planet]);

        let moon_distance = (simulation.position(moon) - simulation.position(planet)).length();
        assert!((moon_distance - 0.1).abs() < 1.0e-6);
    }

    #[test]
    fn moon_forecast_tracks_moon_paths() {
        let mut world = World::default();
        let star = world.spawn(test_star());
        let planet = world.spawn(test_surface_body(
            "Parent",
            CelestialKind::Planet,
            Some(star),
            5.972e24,
            0.1,
            Orbit::circular(1.0, 1.0),
        ));
        let moon = world.spawn(test_surface_body(
            "Child",
            CelestialKind::Moon,
            Some(planet),
            7.342e22,
            0.03,
            Orbit::circular(0.1, 12.0),
        ));

        let simulation = NBodySimulation::from_world(
            &world,
            NBodyConfig {
                years_per_second: 1.0,
                fixed_step_years: 1.0 / 4096.0,
                softening_length: 0.0,
            },
        );

        let forecast = simulation.forecast_full_moon_orbits(16, 1.0 / 512.0);

        assert_eq!(forecast.len(), 1);
        assert_eq!(forecast[0].0, moon);
        assert_eq!(forecast[0].1.len(), 17);
        assert!((forecast[0].1[16] - forecast[0].1[0]).length() > 0.0);
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
                    base_color: Color::rgb(1.0, 0.72, 0.08),
                    accent_color: Color::rgb(1.0, 0.92, 0.2),
                    brightness: 1.0,
                    surface_temperature: 5778.0,
                }),
            },
            atmosphere: None,
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
        }
    }

    fn total_momentum(bodies: &[Body]) -> DVec3 {
        bodies.iter().fold(DVec3::ZERO, |momentum, body| {
            momentum + body.velocity * body.mass
        })
    }
}
