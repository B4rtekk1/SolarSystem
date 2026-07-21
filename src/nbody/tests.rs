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
        accelerations_valid: false,
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
    assert_eq!(NBodyConfig::default().years_per_second, 0.00025);
    assert_eq!(NBodyConfig::default().softening_length, 0.0);
    assert_eq!(NBodyConfig::default().fixed_step_years, 1.0 / 16_384.0);
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
        (accelerations[0].x - GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2 * 0.001).abs() < 1.0e-12
    );
    assert!(accelerations[0].y.abs() < 1.0e-12);
    assert!(accelerations[0].z.abs() < 1.0e-12);
    assert!((accelerations[1].x + GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2).abs() < 1.0e-12);
}

#[test]
fn energy_reports_kinetic_and_potential_joules() {
    let simulation = NBodySimulation {
        bodies: vec![
            Body {
                mass: 1.0,
                position: DVec3::ZERO,
                velocity: DVec3::new(1.0, 0.0, 0.0),
            },
            Body {
                mass: 0.001,
                position: DVec3::X,
                velocity: DVec3::ZERO,
            },
        ],
        body_index_by_entity: Vec::new(),
        planet_entities: Vec::new(),
        moon_orbits: Vec::new(),
        current_accelerations: vec![DVec3::ZERO; 2],
        next_accelerations: vec![DVec3::ZERO; 2],
        accelerations_valid: false,
        config: NBodyConfig {
            years_per_second: 1.0,
            fixed_step_years: 0.01,
            softening_length: 0.0,
        },
        accumulator_years: 0.0,
        elapsed_years: 0.0,
    };

    let energy = simulation.energy();
    let speed_meters_per_second = ASTRONOMICAL_UNIT_METERS / JULIAN_YEAR_SECONDS;
    let expected_kinetic = 0.5 * SOLAR_MASS_KG * speed_meters_per_second.powi(2);
    let expected_potential =
        -GRAVITATIONAL_CONSTANT_M3_KG_S2 * SOLAR_MASS_KG * (0.001 * SOLAR_MASS_KG)
            / ASTRONOMICAL_UNIT_METERS;

    assert!((energy.kinetic_joules / expected_kinetic - 1.0).abs() < 1.0e-12);
    assert!((energy.potential_joules / expected_potential - 1.0).abs() < 1.0e-12);
    assert!((energy.total_joules() - (expected_kinetic + expected_potential)).abs() < 1.0e30);
}

#[test]
fn entity_energy_assigns_half_of_pair_potential() {
    let entity = Entity::from_index(0);
    let simulation = NBodySimulation {
        bodies: vec![
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
        ],
        body_index_by_entity: vec![Some(0), Some(1)],
        planet_entities: Vec::new(),
        moon_orbits: Vec::new(),
        current_accelerations: vec![DVec3::ZERO; 2],
        next_accelerations: vec![DVec3::ZERO; 2],
        accelerations_valid: false,
        config: NBodyConfig {
            years_per_second: 1.0,
            fixed_step_years: 0.01,
            softening_length: 0.0,
        },
        accumulator_years: 0.0,
        elapsed_years: 0.0,
    };

    let total = simulation.energy();
    let entity_energy = simulation.entity_energy(entity).unwrap();

    assert_eq!(entity_energy.kinetic_joules, 0.0);
    assert!(
        (entity_energy.potential_joules / (0.5 * total.potential_joules) - 1.0).abs() < 1.0e-12
    );
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
        (velocity.z - (GRAVITATIONAL_CONSTANT_AU3_SOLAR_MASS_YEAR2 * 1.5).sqrt()).abs() < 1.0e-6
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
        accelerations_valid: false,
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
        accelerations_valid: false,
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
fn forecast_stops_after_full_retrograde_orbit() {
    let moon = Entity::from_index(1);
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
                velocity: -DVec3::Y * TAU,
            },
        ],
        body_index_by_entity: vec![Some(0), Some(1)],
        planet_entities: vec![moon],
        moon_orbits: Vec::new(),
        current_accelerations: vec![DVec3::ZERO; 2],
        next_accelerations: vec![DVec3::ZERO; 2],
        accelerations_valid: false,
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

fn total_momentum(bodies: &[Body]) -> DVec3 {
    bodies.iter().fold(DVec3::ZERO, |momentum, body| {
        momentum + body.velocity * body.mass
    })
}
