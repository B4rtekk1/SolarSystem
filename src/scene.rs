use crate::color::Color;
use crate::constants::{EARTH_MASS_KG, EARTH_RADIUS_KM, LUNAR_MASS_KG, LUNAR_RADIUS_KM, SOLAR_MASS_KG, SOLAR_RADIUS_KM};
use crate::ecs::{AtmosphereComponent, BodyComponent, CelestialKind, Entity, MaterialComponent, ObjectBundle, RenderComponent, RotationComponent, StarMaterial, SurfaceMaterial, World};

pub fn create_world() -> World {
    let mut world = World::default();
    let star = world.spawn(star_bundle());
    let specs = [
        (
            "Aurelia",
            1.05,
            1.0,
            2.7,
            2.05,
            0.55,
            0.00,
            Color::rgb(0.10, 0.34, 1.00),
        ),
        (
            "Vesta",
            0.72,
            0.35,
            1.55,
            1.42,
            0.85,
            0.12,
            Color::rgb(0.85, 0.46, 0.18),
        ),
        (
            "Nereid",
            1.20,
            1.8,
            3.65,
            3.15,
            0.38,
            -0.18,
            Color::rgb(0.22, 0.78, 0.74),
        ),
        (
            "Icarus",
            0.55,
            0.18,
            1.05,
            0.92,
            1.20,
            0.04,
            Color::rgb(0.76, 0.24, 0.12),
        ),
        (
            "Boreas",
            4.20,
            28.0,
            4.65,
            4.10,
            0.25,
            0.28,
            Color::rgb(0.45, 0.68, 0.92),
        ),
        (
            "Nyx",
            2.50,
            6.0,
            5.35,
            4.75,
            0.18,
            -0.32,
            Color::rgb(0.42, 0.36, 0.68),
        ),
    ];

    for (index, (name, earth_radii, earth_masses, major, minor, speed, inclination, color)) in
        specs.into_iter().enumerate()
    {
        let mut orbit = crate::orbit::Orbit::elliptical(major, minor, speed);
        orbit.phase = index as f32 * 0.85;
        orbit.inclination = inclination;

        let planet = world.spawn(ObjectBundle {
            name: name.to_string(),
            kind: CelestialKind::Planet,
            parent: Some(star),
            body: BodyComponent::new(
                EARTH_MASS_KG * earth_masses,
                EARTH_RADIUS_KM * earth_radii,
                Some(orbit),
            ),
            rotation: RotationComponent {
                speed: 0.7 + index as f32 * 0.18,
            },
            render: RenderComponent {
                material: MaterialComponent::Surface(SurfaceMaterial {
                    base_color: color,
                    accent_color: Color::rgb(0.55, 0.85, 1.0),
                    roughness: 0.65 + index as f32 * 0.04,
                    metallic: 0.02,
                }),
            },
            atmosphere: Some(AtmosphereComponent::new(
                Color::rgb(0.45, 0.72, 1.0),
                0.20 + index as f32 * 0.03,
                1.08,
            )),
        });

        for moon in create_moons_for_planet(index, planet) {
            world.spawn(moon);
        }
    }

    world
}

pub fn star_bundle() -> ObjectBundle {
    ObjectBundle {
        name: "Sol".to_string(),
        kind: CelestialKind::Star,
        parent: None,
        body: BodyComponent::new(SOLAR_MASS_KG, SOLAR_RADIUS_KM, None),
        rotation: RotationComponent { speed: 0.15 },
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

pub fn create_moons_for_planet(planet_index: usize, parent: Entity) -> Vec<ObjectBundle> {
    match planet_index {
        0 => vec![make_moon(
            parent,
            "Luma",
            0.85,
            0.36,
            32.0,
            0.18,
            0.40,
            Color::rgb(0.62, 0.63, 0.59),
        )],
        1 => vec![make_moon(
            parent,
            "Cinder",
            0.56,
            0.27,
            -44.0,
            -0.10,
            1.70,
            Color::rgb(0.56, 0.42, 0.34),
        )],
        2 => vec![
            make_moon(
                parent,
                "Nami",
                0.72,
                0.35,
                28.0,
                0.22,
                0.20,
                Color::rgb(0.70, 0.76, 0.78),
            ),
            make_moon(
                parent,
                "Thalassa",
                0.56,
                0.52,
                -18.0,
                -0.16,
                2.40,
                Color::rgb(0.45, 0.58, 0.64),
            ),
        ],
        3 => vec![make_moon(
            parent,
            "Pyra",
            0.48,
            0.23,
            55.0,
            0.05,
            2.80,
            Color::rgb(0.67, 0.50, 0.42),
        )],
        4 => vec![
            make_moon(
                parent,
                "Caldus",
                1.15,
                0.46,
                22.0,
                0.25,
                0.90,
                Color::rgb(0.72, 0.66, 0.56),
            ),
            make_moon(
                parent,
                "Rime",
                0.85,
                0.63,
                -15.0,
                -0.18,
                2.20,
                Color::rgb(0.76, 0.82, 0.88),
            ),
            make_moon(
                parent,
                "Aster",
                0.64,
                0.82,
                10.0,
                0.34,
                3.60,
                Color::rgb(0.50, 0.48, 0.44),
            ),
        ],
        5 => vec![
            make_moon(
                parent,
                "Umbra",
                0.65,
                0.30,
                34.0,
                -0.25,
                1.10,
                Color::rgb(0.40, 0.42, 0.50),
            ),
            make_moon(
                parent,
                "Nyxis",
                0.56,
                0.46,
                -21.0,
                0.20,
                2.90,
                Color::rgb(0.60, 0.58, 0.68),
            ),
        ],
        _ => Vec::new(),
    }
}

pub fn make_moon(
    parent: Entity,
    name: &str,
    lunar_radii: f32,
    orbit_radius: f32,
    angular_speed: f32,
    inclination: f32,
    phase: f32,
    color: Color,
) -> ObjectBundle {
    let mut orbit = crate::orbit::Orbit::circular(orbit_radius, angular_speed);
    orbit.phase = phase;
    orbit.inclination = inclination;

    let mass_scale = lunar_radii.max(0.25).powi(3);
    ObjectBundle {
        name: name.to_string(),
        kind: CelestialKind::Moon,
        parent: Some(parent),
        body: BodyComponent::new(
            LUNAR_MASS_KG * mass_scale,
            LUNAR_RADIUS_KM * lunar_radii,
            Some(orbit),
        ),
        rotation: RotationComponent {
            speed: 0.65 + lunar_radii * 0.25,
        },
        render: RenderComponent {
            material: MaterialComponent::Surface(SurfaceMaterial {
                base_color: color,
                accent_color: Color::rgb(0.70, 0.72, 0.76),
                roughness: 0.88,
                metallic: 0.0,
            }),
        },
        atmosphere: None,
    }
}