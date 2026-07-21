//! The Solar System scene.
//!
//! Orbital elements below are intentionally mean, two-body elements.  They are
//! suitable for the real-time educational simulation, but are not ephemerides.
//! The source values are based on the JPL Solar System Dynamics planetary and
//! satellite physical-parameter/mean-element tables (accessed 2026-07-14).

use crate::color::Color;
use crate::constants::{AU_KM, SOLAR_MASS_KG, SOLAR_RADIUS_KM};
use crate::ecs::{
    AtmosphereComponent, BodyComponent, CelestialKind, Entity, MaterialComponent, ObjectBundle,
    RenderComponent, RingComponent, RotationComponent, StarMaterial, SurfaceMaterial, World,
};
use crate::orbit::Orbit;

#[derive(Clone, Copy)]
struct MoonSpec {
    name: &'static str,
    mass_kg: f32,
    radius_km: f32,
    semi_major_km: f32,
    eccentricity: f32,
    inclination_deg: f32,
}

#[derive(Clone, Copy)]
struct MoonPlane {
    inclination_deg: f32,
    ascending_node_deg: f32,
    periapsis_deg: f32,
}

pub fn create_world() -> World {
    let mut world = World::default();
    let sun = world.spawn(star_bundle());

    let mercury = spawn_planet(
        &mut world,
        sun,
        "Mercury",
        3.3011e23,
        2_439.7,
        0.387_098,
        0.2056,
        7.0,
        Color::rgb(0.48, 0.45, 0.42),
        None,
    );
    let venus = spawn_planet(
        &mut world,
        sun,
        "Venus",
        4.8675e24,
        6_051.8,
        0.723_332,
        0.0068,
        3.4,
        Color::rgb(0.86, 0.67, 0.40),
        Some(AtmosphereComponent::new(
            Color::rgb(0.96, 0.78, 0.45),
            0.24,
            1.06,
        )),
    );
    let earth = spawn_planet(
        &mut world,
        sun,
        "Earth",
        5.97217e24,
        6_371.0,
        1.000_000,
        0.0167,
        0.0,
        Color::rgb(0.10, 0.32, 0.86),
        Some(AtmosphereComponent::new(
            Color::rgb(0.42, 0.70, 1.0),
            0.26,
            1.09,
        )),
    );
    let mars = spawn_planet(
        &mut world,
        sun,
        "Mars",
        6.4171e23,
        3_389.5,
        1.523_679,
        0.0934,
        1.85,
        Color::rgb(0.74, 0.24, 0.12),
        Some(AtmosphereComponent::new(
            Color::rgb(0.94, 0.42, 0.24),
            0.07,
            1.03,
        )),
    );
    let jupiter = spawn_planet(
        &mut world,
        sun,
        "Jupiter",
        1.89813e27,
        69_911.0,
        5.2044,
        0.0489,
        1.30,
        Color::rgb(0.76, 0.58, 0.40),
        Some(AtmosphereComponent::new(
            Color::rgb(0.96, 0.78, 0.58),
            0.18,
            1.04,
        )),
    );
    let saturn = spawn_planet(
        &mut world,
        sun,
        "Saturn",
        5.6834e26,
        58_232.0,
        9.5826,
        0.0565,
        2.49,
        Color::rgb(0.82, 0.73, 0.54),
        Some(AtmosphereComponent::new(
            Color::rgb(0.94, 0.84, 0.62),
            0.15,
            1.04,
        )),
    );
    let uranus = spawn_planet(
        &mut world,
        sun,
        "Uranus",
        8.6810e25,
        25_362.0,
        19.2184,
        0.0463,
        0.77,
        Color::rgb(0.45, 0.82, 0.85),
        Some(AtmosphereComponent::new(
            Color::rgb(0.58, 0.90, 0.92),
            0.18,
            1.04,
        )),
    );
    let neptune = spawn_planet(
        &mut world,
        sun,
        "Neptune",
        1.02413e26,
        24_622.0,
        30.1104,
        0.0095,
        1.77,
        Color::rgb(0.15, 0.30, 0.82),
        Some(AtmosphereComponent::new(
            Color::rgb(0.32, 0.54, 1.0),
            0.20,
            1.05,
        )),
    );

    add_saturn_ring(&mut world, saturn);

    // IAU-recognised dwarf planets.  They share the planet renderer because this
    // ECS currently has one non-stellar primary-body type.
    let ceres = spawn_planet(
        &mut world,
        sun,
        "Ceres (dwarf planet)",
        9.393e20,
        473.0,
        2.7675,
        0.0758,
        10.59,
        Color::rgb(0.48, 0.47, 0.43),
        None,
    );
    let pluto = spawn_planet(
        &mut world,
        sun,
        "Pluto (dwarf planet)",
        1.303e22,
        1_188.3,
        39.482,
        0.2488,
        17.16,
        Color::rgb(0.67, 0.57, 0.49),
        None,
    );
    let haumea = spawn_planet(
        &mut world,
        sun,
        "Haumea (dwarf planet)",
        4.006e21,
        816.0,
        43.218,
        0.1913,
        28.19,
        Color::rgb(0.82, 0.82, 0.78),
        None,
    );
    let makemake = spawn_planet(
        &mut world,
        sun,
        "Makemake (dwarf planet)",
        3.1e21,
        715.0,
        45.430,
        0.159,
        28.96,
        Color::rgb(0.72, 0.48, 0.30),
        None,
    );
    let eris = spawn_planet(
        &mut world,
        sun,
        "Eris (dwarf planet)",
        1.6466e22,
        1_163.0,
        67.668,
        0.4418,
        44.04,
        Color::rgb(0.83, 0.84, 0.87),
        None,
    );
    let _ = (mercury, venus, ceres, makemake); // Primaries with no confirmed moons.

    spawn_moons(
        &mut world,
        earth,
        &EARTH_MOONS,
        MOON_PLANE_EARTH,
        Color::rgb(0.66, 0.66, 0.63),
    );
    spawn_moons(
        &mut world,
        mars,
        &MARS_MOONS,
        MOON_PLANE_MARS,
        Color::rgb(0.55, 0.43, 0.35),
    );
    spawn_moons(
        &mut world,
        jupiter,
        &JUPITER_MOONS,
        MOON_PLANE_JUPITER,
        Color::rgb(0.72, 0.66, 0.54),
    );
    spawn_moons(
        &mut world,
        saturn,
        &SATURN_MOONS,
        MOON_PLANE_SATURN,
        Color::rgb(0.74, 0.71, 0.62),
    );
    spawn_moons(
        &mut world,
        uranus,
        &URANUS_MOONS,
        MOON_PLANE_URANUS,
        Color::rgb(0.60, 0.70, 0.74),
    );
    spawn_moons(
        &mut world,
        neptune,
        &NEPTUNE_MOONS,
        MOON_PLANE_NEPTUNE,
        Color::rgb(0.56, 0.61, 0.70),
    );
    spawn_moons(
        &mut world,
        pluto,
        &PLUTO_MOONS,
        MOON_PLANE_PLUTO,
        Color::rgb(0.68, 0.66, 0.62),
    );
    spawn_moons(
        &mut world,
        haumea,
        &HAUMEA_MOONS,
        MOON_PLANE_HAUMEA,
        Color::rgb(0.72, 0.76, 0.78),
    );
    spawn_moons(
        &mut world,
        eris,
        &ERIS_MOONS,
        MOON_PLANE_ERIS,
        Color::rgb(0.66, 0.67, 0.72),
    );

    world
}

fn spawn_planet(
    world: &mut World,
    parent: Entity,
    name: &str,
    mass_kg: f32,
    radius_km: f32,
    semi_major_au: f32,
    eccentricity: f32,
    inclination_deg: f32,
    color: Color,
    atmosphere: Option<AtmosphereComponent>,
) -> Entity {
    let mut orbit = Orbit::elliptical(
        semi_major_au,
        semi_major_au * (1.0 - eccentricity * eccentricity).sqrt(),
        0.0,
    );
    orbit.inclination = inclination_deg.to_radians();
    let (ascending_node_deg, periapsis_deg) = primary_orbit_orientation(name);
    orbit.ascending_node = ascending_node_deg.to_radians();
    orbit.argument_of_periapsis = periapsis_deg.to_radians();
    // Spread initial positions around their real orbits to make the scene legible.
    orbit.phase = (semi_major_au * 1.37).rem_euclid(std::f32::consts::TAU);
    world.spawn(ObjectBundle {
        name: name.to_owned(),
        kind: CelestialKind::Planet,
        parent: Some(parent),
        body: BodyComponent::new(mass_kg, radius_km, Some(orbit)),
        rotation: RotationComponent { speed: 0.18 },
        render: RenderComponent {
            material: MaterialComponent::Surface(SurfaceMaterial {
                base_color: color,
                accent_color: Color::rgb(0.72, 0.82, 0.92),
                roughness: 0.72,
                metallic: 0.0,
            }),
        },
        atmosphere,
        ring: None,
    })
}

fn primary_orbit_orientation(name: &str) -> (f32, f32) {
    match name {
        "Mercury" => (48.331, 29.124),
        "Venus" => (76.680, 54.884),
        "Earth" => (-11.260, 114.207),
        "Mars" => (49.558, 286.502),
        "Jupiter" => (100.464, 273.867),
        "Saturn" => (113.665, 339.392),
        "Uranus" => (74.006, 96.998),
        "Neptune" => (131.784, 273.187),
        "Ceres (dwarf planet)" => (80.305, 73.597),
        "Pluto (dwarf planet)" => (110.299, 113.834),
        "Haumea (dwarf planet)" => (122.0, 240.0),
        "Makemake (dwarf planet)" => (79.6, 294.8),
        "Eris (dwarf planet)" => (35.9, 151.6),
        _ => (0.0, 0.0),
    }
}

fn spawn_moons(
    world: &mut World,
    parent: Entity,
    moons: &[MoonSpec],
    plane: MoonPlane,
    color: Color,
) {
    for (index, moon) in moons.iter().enumerate() {
        let semi_major_au = moon.semi_major_km as f64 / AU_KM;
        let mut orbit = Orbit::elliptical(
            semi_major_au as f32,
            (semi_major_au * (1.0 - (moon.eccentricity as f64).powi(2)).sqrt()) as f32,
            0.0,
        );
        orbit.inclination = (plane.inclination_deg + moon.inclination_deg).to_radians();
        orbit.ascending_node = plane.ascending_node_deg.to_radians();
        orbit.argument_of_periapsis = (plane.periapsis_deg + index as f32 * 17.0).to_radians();
        orbit.phase = (index as f32 * 2.399_963).rem_euclid(std::f32::consts::TAU);
        world.spawn(ObjectBundle {
            name: moon.name.to_owned(),
            kind: CelestialKind::Moon,
            parent: Some(parent),
            body: BodyComponent::new(moon.mass_kg, moon.radius_km, Some(orbit)),
            rotation: RotationComponent { speed: 0.35 },
            render: RenderComponent {
                material: MaterialComponent::Surface(SurfaceMaterial {
                    base_color: color,
                    accent_color: Color::rgb(0.78, 0.80, 0.83),
                    roughness: 0.9,
                    metallic: 0.0,
                }),
            },
            atmosphere: None,
            ring: None,
        });
    }
}

fn add_saturn_ring(world: &mut World, saturn: Entity) {
    world.ring_mut(saturn).replace(RingComponent::new(
        1.22,
        2.35,
        0.42,
        0.35,
        Color::rgb(0.78, 0.72, 0.58),
        5_000,
    ));
}

pub fn star_bundle() -> ObjectBundle {
    ObjectBundle {
        name: "Sun".to_string(),
        kind: CelestialKind::Star,
        parent: None,
        body: BodyComponent::new(SOLAR_MASS_KG, SOLAR_RADIUS_KM, None),
        rotation: RotationComponent { speed: 0.15 },
        render: RenderComponent {
            material: MaterialComponent::Star(StarMaterial {
                base_color: Color::rgb(1.0, 0.72, 0.08),
                accent_color: Color::rgb(1.0, 0.92, 0.2),
                brightness: 1.0,
                surface_temperature: 5_778.0,
            }),
        },
        atmosphere: None,
        ring: None,
    }
}

// Reference-plane orientation for the regular satellite systems.  The giant
// planets' moons orbit close to their equators, so their systems must not be
// drawn in Earth's ecliptic plane.
const MOON_PLANE_EARTH: MoonPlane = MoonPlane {
    inclination_deg: 0.0,
    ascending_node_deg: 125.08,
    periapsis_deg: 318.15,
};
const MOON_PLANE_MARS: MoonPlane = MoonPlane {
    inclination_deg: 25.19,
    ascending_node_deg: 49.56,
    periapsis_deg: 250.0,
};
const MOON_PLANE_JUPITER: MoonPlane = MoonPlane {
    inclination_deg: 3.13,
    ascending_node_deg: 100.56,
    periapsis_deg: 275.0,
};
const MOON_PLANE_SATURN: MoonPlane = MoonPlane {
    inclination_deg: 26.73,
    ascending_node_deg: 113.72,
    periapsis_deg: 339.0,
};
const MOON_PLANE_URANUS: MoonPlane = MoonPlane {
    inclination_deg: 97.77,
    ascending_node_deg: 74.01,
    periapsis_deg: 97.0,
};
const MOON_PLANE_NEPTUNE: MoonPlane = MoonPlane {
    inclination_deg: 28.32,
    ascending_node_deg: 131.78,
    periapsis_deg: 273.0,
};
const MOON_PLANE_PLUTO: MoonPlane = MoonPlane {
    inclination_deg: 119.61,
    ascending_node_deg: 110.30,
    periapsis_deg: 113.8,
};
const MOON_PLANE_HAUMEA: MoonPlane = MoonPlane {
    inclination_deg: 126.0,
    ascending_node_deg: 122.0,
    periapsis_deg: 240.0,
};
const MOON_PLANE_ERIS: MoonPlane = MoonPlane {
    inclination_deg: 78.0,
    ascending_node_deg: 35.0,
    periapsis_deg: 20.0,
};

const EARTH_MOONS: [MoonSpec; 1] = [MoonSpec {
    name: "Moon",
    mass_kg: 7.342e22,
    radius_km: 1_737.4,
    semi_major_km: 384_400.0,
    eccentricity: 0.0549,
    inclination_deg: 5.15,
}];
const MARS_MOONS: [MoonSpec; 2] = [
    MoonSpec {
        name: "Phobos",
        mass_kg: 1.0659e16,
        radius_km: 11.27,
        semi_major_km: 9_376.0,
        eccentricity: 0.0151,
        inclination_deg: 1.08,
    },
    MoonSpec {
        name: "Deimos",
        mass_kg: 1.4762e15,
        radius_km: 6.2,
        semi_major_km: 23_463.0,
        eccentricity: 0.0002,
        inclination_deg: 1.79,
    },
];
const JUPITER_MOONS: [MoonSpec; 8] = [
    MoonSpec {
        name: "Metis",
        mass_kg: 3.6e16,
        radius_km: 21.5,
        semi_major_km: 128_000.0,
        eccentricity: 0.0002,
        inclination_deg: 0.06,
    },
    MoonSpec {
        name: "Adrastea",
        mass_kg: 2.0e15,
        radius_km: 8.2,
        semi_major_km: 129_000.0,
        eccentricity: 0.0018,
        inclination_deg: 0.03,
    },
    MoonSpec {
        name: "Amalthea",
        mass_kg: 2.08e18,
        radius_km: 83.5,
        semi_major_km: 181_366.0,
        eccentricity: 0.0032,
        inclination_deg: 0.37,
    },
    MoonSpec {
        name: "Thebe",
        mass_kg: 4.3e17,
        radius_km: 49.3,
        semi_major_km: 221_889.0,
        eccentricity: 0.0175,
        inclination_deg: 1.08,
    },
    MoonSpec {
        name: "Io",
        mass_kg: 8.9319e22,
        radius_km: 1_821.6,
        semi_major_km: 421_700.0,
        eccentricity: 0.0041,
        inclination_deg: 0.04,
    },
    MoonSpec {
        name: "Europa",
        mass_kg: 4.7998e22,
        radius_km: 1_560.8,
        semi_major_km: 671_034.0,
        eccentricity: 0.0094,
        inclination_deg: 0.47,
    },
    MoonSpec {
        name: "Ganymede",
        mass_kg: 1.4819e23,
        radius_km: 2_634.1,
        semi_major_km: 1_070_412.0,
        eccentricity: 0.0013,
        inclination_deg: 0.20,
    },
    MoonSpec {
        name: "Callisto",
        mass_kg: 1.0759e23,
        radius_km: 2_410.3,
        semi_major_km: 1_882_709.0,
        eccentricity: 0.0074,
        inclination_deg: 0.19,
    },
];
const SATURN_MOONS: [MoonSpec; 16] = [
    MoonSpec {
        name: "Pan",
        mass_kg: 4.95e15,
        radius_km: 14.0,
        semi_major_km: 133_584.0,
        eccentricity: 0.0000,
        inclination_deg: 0.0,
    },
    MoonSpec {
        name: "Atlas",
        mass_kg: 6.6e15,
        radius_km: 15.1,
        semi_major_km: 137_670.0,
        eccentricity: 0.0012,
        inclination_deg: 0.0,
    },
    MoonSpec {
        name: "Prometheus",
        mass_kg: 1.6e17,
        radius_km: 43.1,
        semi_major_km: 139_380.0,
        eccentricity: 0.0022,
        inclination_deg: 0.0,
    },
    MoonSpec {
        name: "Pandora",
        mass_kg: 1.4e17,
        radius_km: 40.6,
        semi_major_km: 141_720.0,
        eccentricity: 0.0042,
        inclination_deg: 0.0,
    },
    MoonSpec {
        name: "Epimetheus",
        mass_kg: 5.3e17,
        radius_km: 58.1,
        semi_major_km: 151_422.0,
        eccentricity: 0.0098,
        inclination_deg: 0.35,
    },
    MoonSpec {
        name: "Janus",
        mass_kg: 1.90e18,
        radius_km: 89.5,
        semi_major_km: 151_472.0,
        eccentricity: 0.0068,
        inclination_deg: 0.16,
    },
    MoonSpec {
        name: "Mimas",
        mass_kg: 3.75e19,
        radius_km: 198.2,
        semi_major_km: 185_539.0,
        eccentricity: 0.0196,
        inclination_deg: 1.57,
    },
    MoonSpec {
        name: "Enceladus",
        mass_kg: 1.08e20,
        radius_km: 252.1,
        semi_major_km: 238_042.0,
        eccentricity: 0.0047,
        inclination_deg: 0.01,
    },
    MoonSpec {
        name: "Tethys",
        mass_kg: 6.17e20,
        radius_km: 531.1,
        semi_major_km: 294_672.0,
        eccentricity: 0.0001,
        inclination_deg: 1.09,
    },
    MoonSpec {
        name: "Dione",
        mass_kg: 1.095e21,
        radius_km: 561.4,
        semi_major_km: 377_415.0,
        eccentricity: 0.0022,
        inclination_deg: 0.02,
    },
    MoonSpec {
        name: "Rhea",
        mass_kg: 2.307e21,
        radius_km: 763.8,
        semi_major_km: 527_068.0,
        eccentricity: 0.0010,
        inclination_deg: 0.35,
    },
    MoonSpec {
        name: "Titan",
        mass_kg: 1.3452e23,
        radius_km: 2_574.7,
        semi_major_km: 1_221_870.0,
        eccentricity: 0.0288,
        inclination_deg: 0.33,
    },
    MoonSpec {
        name: "Hyperion",
        mass_kg: 5.6e18,
        radius_km: 135.0,
        semi_major_km: 1_481_100.0,
        eccentricity: 0.1042,
        inclination_deg: 0.43,
    },
    MoonSpec {
        name: "Iapetus",
        mass_kg: 1.806e21,
        radius_km: 734.5,
        semi_major_km: 3_560_820.0,
        eccentricity: 0.0286,
        inclination_deg: 15.47,
    },
    MoonSpec {
        name: "Phoebe",
        mass_kg: 8.29e18,
        radius_km: 106.5,
        semi_major_km: 12_952_000.0,
        eccentricity: 0.163,
        inclination_deg: 175.2,
    },
    MoonSpec {
        name: "Aegir",
        mass_kg: 2.0e15,
        radius_km: 3.0,
        semi_major_km: 20_735_000.0,
        eccentricity: 0.25,
        inclination_deg: 167.0,
    },
];
const URANUS_MOONS: [MoonSpec; 15] = [
    MoonSpec {
        name: "Cordelia",
        mass_kg: 4.4e16,
        radius_km: 20.1,
        semi_major_km: 49_752.0,
        eccentricity: 0.0003,
        inclination_deg: 0.08,
    },
    MoonSpec {
        name: "Ophelia",
        mass_kg: 5.3e16,
        radius_km: 21.4,
        semi_major_km: 53_764.0,
        eccentricity: 0.0099,
        inclination_deg: 0.10,
    },
    MoonSpec {
        name: "Bianca",
        mass_kg: 9.2e16,
        radius_km: 25.7,
        semi_major_km: 59_166.0,
        eccentricity: 0.0009,
        inclination_deg: 0.19,
    },
    MoonSpec {
        name: "Cressida",
        mass_kg: 3.4e17,
        radius_km: 39.8,
        semi_major_km: 61_767.0,
        eccentricity: 0.0004,
        inclination_deg: 0.01,
    },
    MoonSpec {
        name: "Desdemona",
        mass_kg: 1.8e17,
        radius_km: 32.0,
        semi_major_km: 62_658.0,
        eccentricity: 0.0001,
        inclination_deg: 0.11,
    },
    MoonSpec {
        name: "Juliet",
        mass_kg: 5.6e17,
        radius_km: 46.8,
        semi_major_km: 64_358.0,
        eccentricity: 0.0007,
        inclination_deg: 0.07,
    },
    MoonSpec {
        name: "Portia",
        mass_kg: 1.7e18,
        radius_km: 67.6,
        semi_major_km: 66_097.0,
        eccentricity: 0.0001,
        inclination_deg: 0.06,
    },
    MoonSpec {
        name: "Rosalind",
        mass_kg: 2.5e17,
        radius_km: 36.0,
        semi_major_km: 69_927.0,
        eccentricity: 0.0001,
        inclination_deg: 0.28,
    },
    MoonSpec {
        name: "Belinda",
        mass_kg: 3.6e17,
        radius_km: 40.3,
        semi_major_km: 75_255.0,
        eccentricity: 0.0001,
        inclination_deg: 0.03,
    },
    MoonSpec {
        name: "Puck",
        mass_kg: 2.9e18,
        radius_km: 81.0,
        semi_major_km: 86_004.0,
        eccentricity: 0.0001,
        inclination_deg: 0.32,
    },
    MoonSpec {
        name: "Miranda",
        mass_kg: 6.59e19,
        radius_km: 235.8,
        semi_major_km: 129_390.0,
        eccentricity: 0.0013,
        inclination_deg: 4.34,
    },
    MoonSpec {
        name: "Ariel",
        mass_kg: 1.353e21,
        radius_km: 578.9,
        semi_major_km: 190_900.0,
        eccentricity: 0.0012,
        inclination_deg: 0.26,
    },
    MoonSpec {
        name: "Umbriel",
        mass_kg: 1.172e21,
        radius_km: 584.7,
        semi_major_km: 266_000.0,
        eccentricity: 0.0039,
        inclination_deg: 0.13,
    },
    MoonSpec {
        name: "Titania",
        mass_kg: 3.527e21,
        radius_km: 788.9,
        semi_major_km: 435_910.0,
        eccentricity: 0.0011,
        inclination_deg: 0.08,
    },
    MoonSpec {
        name: "Oberon",
        mass_kg: 3.014e21,
        radius_km: 761.4,
        semi_major_km: 583_520.0,
        eccentricity: 0.0014,
        inclination_deg: 0.10,
    },
];
const NEPTUNE_MOONS: [MoonSpec; 9] = [
    MoonSpec {
        name: "Naiad",
        mass_kg: 1.9e17,
        radius_km: 33.0,
        semi_major_km: 48_227.0,
        eccentricity: 0.0003,
        inclination_deg: 4.7,
    },
    MoonSpec {
        name: "Thalassa",
        mass_kg: 3.7e17,
        radius_km: 41.0,
        semi_major_km: 50_075.0,
        eccentricity: 0.0002,
        inclination_deg: 0.2,
    },
    MoonSpec {
        name: "Despina",
        mass_kg: 2.1e18,
        radius_km: 75.0,
        semi_major_km: 52_526.0,
        eccentricity: 0.0004,
        inclination_deg: 0.1,
    },
    MoonSpec {
        name: "Galatea",
        mass_kg: 2.1e18,
        radius_km: 88.0,
        semi_major_km: 61_953.0,
        eccentricity: 0.0002,
        inclination_deg: 0.1,
    },
    MoonSpec {
        name: "Larissa",
        mass_kg: 4.2e18,
        radius_km: 97.0,
        semi_major_km: 73_548.0,
        eccentricity: 0.0014,
        inclination_deg: 0.2,
    },
    MoonSpec {
        name: "Hippocamp",
        mass_kg: 1.0e16,
        radius_km: 17.4,
        semi_major_km: 105_283.0,
        eccentricity: 0.0005,
        inclination_deg: 0.1,
    },
    MoonSpec {
        name: "Proteus",
        mass_kg: 5.03e19,
        radius_km: 210.0,
        semi_major_km: 117_647.0,
        eccentricity: 0.0005,
        inclination_deg: 0.5,
    },
    MoonSpec {
        name: "Triton",
        mass_kg: 2.139e22,
        radius_km: 1_353.4,
        semi_major_km: 354_759.0,
        eccentricity: 0.0000,
        inclination_deg: 156.9,
    },
    MoonSpec {
        name: "Nereid",
        mass_kg: 3.1e19,
        radius_km: 170.0,
        semi_major_km: 5_513_400.0,
        eccentricity: 0.751,
        inclination_deg: 7.2,
    },
];
const PLUTO_MOONS: [MoonSpec; 5] = [
    MoonSpec {
        name: "Charon",
        mass_kg: 1.586e21,
        radius_km: 606.0,
        semi_major_km: 19_596.0,
        eccentricity: 0.0002,
        inclination_deg: 0.0,
    },
    MoonSpec {
        name: "Styx",
        mass_kg: 7.5e15,
        radius_km: 5.2,
        semi_major_km: 42_656.0,
        eccentricity: 0.0058,
        inclination_deg: 0.81,
    },
    MoonSpec {
        name: "Nix",
        mass_kg: 4.5e16,
        radius_km: 19.3,
        semi_major_km: 48_694.0,
        eccentricity: 0.0020,
        inclination_deg: 0.13,
    },
    MoonSpec {
        name: "Kerberos",
        mass_kg: 1.6e16,
        radius_km: 6.0,
        semi_major_km: 57_783.0,
        eccentricity: 0.0033,
        inclination_deg: 0.39,
    },
    MoonSpec {
        name: "Hydra",
        mass_kg: 4.8e16,
        radius_km: 25.0,
        semi_major_km: 64_738.0,
        eccentricity: 0.0059,
        inclination_deg: 0.24,
    },
];
const HAUMEA_MOONS: [MoonSpec; 2] = [
    MoonSpec {
        name: "Hi'iaka",
        mass_kg: 1.79e19,
        radius_km: 160.0,
        semi_major_km: 49_880.0,
        eccentricity: 0.051,
        inclination_deg: 0.0,
    },
    MoonSpec {
        name: "Namaka",
        mass_kg: 1.79e18,
        radius_km: 85.0,
        semi_major_km: 25_657.0,
        eccentricity: 0.249,
        inclination_deg: 13.4,
    },
];
const ERIS_MOONS: [MoonSpec; 1] = [MoonSpec {
    name: "Dysnomia",
    mass_kg: 8.2e19,
    radius_km: 350.0,
    semi_major_km: 37_273.0,
    eccentricity: 0.014,
    inclination_deg: 0.0,
}];

#[cfg(test)]
mod tests;
