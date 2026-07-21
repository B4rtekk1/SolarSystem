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
