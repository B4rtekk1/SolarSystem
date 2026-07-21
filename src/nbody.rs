use crate::{
    ecs::{CelestialKind, Entity, World},
    orbit::Orbit,
};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

const GRAVITATIONAL_CONSTANT_M3_KG_S2: f64 = 6.674_30e-11;
const SOLAR_MASS_KG: f64 = 1.988_47e30;
const ASTRONOMICAL_UNIT_METERS: f64 = 149_597_870_700.0;
const JULIAN_YEAR_SECONDS: f64 = 31_557_600.0;
const GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2: f64 =
    GRAVITATIONAL_CONSTANT_M3_KG_S2 * SOLAR_MASS_KG * JULIAN_YEAR_SECONDS * JULIAN_YEAR_SECONDS
        / (ASTRONOMICAL_UNIT_METERS * ASTRONOMICAL_UNIT_METERS * ASTRONOMICAL_UNIT_METERS);
const MAX_FRAME_SECONDS: f64 = 0.1;
const MAX_STEPS_PER_FRAME: usize = 384;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NBodyConfig {
    pub years_per_second: f64,
    pub fixed_step_years: f64,
    pub softening_length: f64,
}

impl Default for NBodyConfig {
    fn default() -> Self {
        Self {
            // Keep the default visual pace low enough for short real moon
            // orbits while still allowing the UI speed multiplier to accelerate
            // the broader planetary system.
            years_per_second: 0.00025,
            // About 32 minutes per step: enough to resolve the innermost
            // real moons substantially better than the previous two-hour step.
            fixed_step_years: 1.0 / 16_384.0,
            softening_length: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Body {
    mass: f64,
    position: DVec3,
    velocity: DVec3,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct EnergySnapshot {
    pub kinetic_joules: f64,
    pub potential_joules: f64,
}

impl EnergySnapshot {
    pub fn total_joules(self) -> f64 {
        self.kinetic_joules + self.potential_joules
    }
}

#[derive(Debug, Clone, Copy)]
struct OrbitForecastTracker {
    normal: DVec3,
    previous_direction: DVec3,
    accumulated_angle: f64,
    complete: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct MoonOrbitTarget {
    entity: Entity,
    parent: Entity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NBodySimulation {
    bodies: Vec<Body>,
    body_index_by_entity: Vec<Option<usize>>,
    planet_entities: Vec<Entity>,
    moon_orbits: Vec<MoonOrbitTarget>,
    current_accelerations: Vec<DVec3>,
    next_accelerations: Vec<DVec3>,
    #[serde(default)]
    accelerations_valid: bool,
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
            accelerations_valid: false,
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

    pub fn validate_for_world(&self, world: &World) -> Result<(), String> {
        let entity_count = world.entity_capacity();
        if self.body_index_by_entity.len() != entity_count {
            return Err("Physics entity index length does not match world".to_string());
        }
        if self.current_accelerations.len() != self.bodies.len()
            || self.next_accelerations.len() != self.bodies.len()
        {
            return Err("Physics acceleration buffers do not match bodies".to_string());
        }
        if !self.config.years_per_second.is_finite()
            || !self.config.fixed_step_years.is_finite()
            || !self.config.softening_length.is_finite()
            || self.config.years_per_second <= 0.0
            || self.config.fixed_step_years <= 0.0
            || self.config.softening_length < 0.0
            || !self.accumulator_years.is_finite()
            || !self.elapsed_years.is_finite()
        {
            return Err("Physics configuration contains invalid values".to_string());
        }

        let mut seen_body_indices = vec![false; self.bodies.len()];
        for entity in world.entities() {
            let Some(body_index) = self.body_index_by_entity[entity.index()] else {
                return Err(format!("Entity {} is missing from physics", entity.index()));
            };
            if body_index >= self.bodies.len() {
                return Err(format!(
                    "Entity {} references missing physics body {}",
                    entity.index(),
                    body_index
                ));
            }
            if seen_body_indices[body_index] {
                return Err(format!(
                    "Physics body {} is assigned to more than one entity",
                    body_index
                ));
            }
            seen_body_indices[body_index] = true;
        }
        if seen_body_indices.iter().any(|seen| !seen) {
            return Err("Physics contains bodies without matching entities".to_string());
        }

        for (index, body) in self.bodies.iter().enumerate() {
            if !body.mass.is_finite()
                || body.mass < 0.0
                || !body.position.is_finite()
                || !body.velocity.is_finite()
            {
                return Err(format!("Physics body {index} contains invalid values"));
            }
        }
        for acceleration in self
            .current_accelerations
            .iter()
            .chain(self.next_accelerations.iter())
        {
            if !acceleration.is_finite() {
                return Err("Physics acceleration buffer contains invalid values".to_string());
            }
        }

        for entity in &self.planet_entities {
            if entity.index() >= entity_count {
                return Err(format!(
                    "Physics references missing planet entity {}",
                    entity.index()
                ));
            }
            if world.kind(*entity) != CelestialKind::Planet {
                return Err(format!(
                    "Physics planet list contains non-planet entity {}",
                    entity.index()
                ));
            }
        }
        for target in &self.moon_orbits {
            if target.entity.index() >= entity_count || target.parent.index() >= entity_count {
                return Err("Physics moon orbit references missing entity".to_string());
            }
            if world.kind(target.entity) != CelestialKind::Moon
                || world.kind(target.parent) != CelestialKind::Planet
                || world
                    .parent(target.entity)
                    .is_none_or(|parent| parent.entity != target.parent)
            {
                return Err(format!(
                    "Physics moon orbit target {} does not match world parentage",
                    target.entity.index()
                ));
            }
        }

        Ok(())
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

    pub fn reset_visual_pacing_to_defaults(&mut self) {
        let default_config = NBodyConfig::default();
        self.config.years_per_second = default_config.years_per_second;
        self.config.fixed_step_years = default_config.fixed_step_years;
        self.accumulator_years = self.accumulator_years.min(self.config.fixed_step_years);
    }

    pub fn energy(&self) -> EnergySnapshot {
        let velocity_scale = ASTRONOMICAL_UNIT_METERS / JULIAN_YEAR_SECONDS;
        let mut kinetic_joules = 0.0;
        for body in &self.bodies {
            kinetic_joules += kinetic_energy_joules(body, velocity_scale);
        }

        let mut potential_joules = 0.0;
        for i in 0..self.bodies.len() {
            for j in (i + 1)..self.bodies.len() {
                potential_joules += pair_potential_energy_joules(
                    &self.bodies[i],
                    &self.bodies[j],
                    self.config.softening_length,
                );
            }
        }

        EnergySnapshot {
            kinetic_joules,
            potential_joules,
        }
    }

    pub fn entity_energy(&self, entity: Entity) -> Option<EnergySnapshot> {
        let body_index = self
            .body_index_by_entity
            .get(entity.index())
            .and_then(|body_index| *body_index)?;
        let body = self.bodies.get(body_index)?;
        let velocity_scale = ASTRONOMICAL_UNIT_METERS / JULIAN_YEAR_SECONDS;
        let kinetic_joules = kinetic_energy_joules(body, velocity_scale);
        let potential_joules = self
            .bodies
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != body_index)
            .map(|(_, other)| {
                0.5 * pair_potential_energy_joules(body, other, self.config.softening_length)
            })
            .sum();

        Some(EnergySnapshot {
            kinetic_joules,
            potential_joules,
        })
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

    fn step(&mut self, dt: f64) {
        self.ensure_acceleration_buffers();
        if !self.accelerations_valid {
            write_accelerations(
                &self.bodies,
                self.config.softening_length,
                &mut self.current_accelerations,
            );
            self.accelerations_valid = true;
        }
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

        // In velocity Verlet the acceleration at the end of this step is the
        // acceleration at the beginning of the next one. Reusing it cuts the
        // dominant O(n²) force calculation almost in half.
        std::mem::swap(
            &mut self.current_accelerations,
            &mut self.next_accelerations,
        );

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

fn kinetic_energy_joules(body: &Body, velocity_scale: f64) -> f64 {
    let mass_kg = body.mass * SOLAR_MASS_KG;
    let speed_meters_per_second = body.velocity.length() * velocity_scale;
    0.5 * mass_kg * speed_meters_per_second * speed_meters_per_second
}

fn pair_potential_energy_joules(a: &Body, b: &Body, softening_length: f64) -> f64 {
    let softening_squared = if softening_length.is_finite() && softening_length > 0.0 {
        softening_length * softening_length
    } else {
        0.0
    };
    let distance_au = ((b.position - a.position).length_squared() + softening_squared).sqrt();
    if distance_au <= f64::EPSILON {
        return 0.0;
    }

    let mass_a_kg = a.mass * SOLAR_MASS_KG;
    let mass_b_kg = b.mass * SOLAR_MASS_KG;
    -GRAVITATIONAL_CONSTANT_M3_KG_S2 * mass_a_kg * mass_b_kg
        / (distance_au * ASTRONOMICAL_UNIT_METERS)
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
    let (sin_periapsis, cos_periapsis) = (orbit.argument_of_periapsis as f64).sin_cos();
    let periapsis_x = x * cos_periapsis - z * sin_periapsis;
    let periapsis_z = x * sin_periapsis + z * cos_periapsis;
    let (sin_inclination, cos_inclination) = (orbit.inclination as f64).sin_cos();
    let inclined = DVec3::new(
        periapsis_x,
        -periapsis_z * sin_inclination,
        periapsis_z * cos_inclination,
    );
    let (sin_node, cos_node) = (orbit.ascending_node as f64).sin_cos();
    DVec3::new(
        inclined.x * cos_node + inclined.z * sin_node,
        inclined.y,
        -inclined.x * sin_node + inclined.z * cos_node,
    )
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
mod tests;
