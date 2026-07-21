use crate::constants::{AU_KM, AU_PER_YEAR_TO_KM_PER_SECOND};
use crate::ecs::{CelestialKind, Entity, MaterialComponent, World};
use crate::nbody::{EnergySnapshot, NBodySimulation};

pub fn au_to_km(au: f64) -> f64 {
    au * AU_KM
}

fn estimate_equilibrium_temperature_k(distance_au: f64, star_brightness: f64) -> f64 {
    // Calibrated so that at 1 AU and brightness=1.0 we get ~288 K (Earth-like).
    // Uses a simplified equilibrium model: T ∝ L^(1/4) / sqrt(r).
    const EARTH_REFERENCE_TEMPERATURE_K: f64 = 288.0;
    let r = distance_au.abs().max(1.0e-9);
    let l = star_brightness.max(0.0);
    EARTH_REFERENCE_TEMPERATURE_K * l.powf(0.25) / r.sqrt()
}

fn apply_atmosphere_greenhouse(base_temperature_k: f64, atmosphere_density: Option<f64>) -> f64 {
    // Simple greenhouse approximation driven by density.
    // density=None -> no adjustment.
    // density=1.0 -> +20% temperature (tunable, but stable and monotonic).
    let density = atmosphere_density.unwrap_or(0.0);
    if !base_temperature_k.is_finite() || !density.is_finite() {
        return base_temperature_k;
    }
    let multiplier = (1.0 + 0.20 * density).clamp(0.0, 3.0);
    base_temperature_k * multiplier
}

fn format_temperature(kelvin: f64) -> String {
    let celsius = kelvin - 273.15;
    if kelvin.is_finite() && celsius.is_finite() {
        format!("{kelvin:.1} K ({celsius:.1} °C)")
    } else {
        "N/A".to_string()
    }
}

pub fn format_energy_joules(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.6e} J")
    } else {
        "N/A".to_string()
    }
}

fn show_energy_labels(
    ui: &mut egui::Ui,
    energy: EnergySnapshot,
    initial_total_energy: Option<f64>,
) {
    ui.label(format!(
        "Total energy: {}",
        format_energy_joules(energy.total_joules())
    ));
    if let Some(initial_total_energy) = initial_total_energy {
        ui.label(format!(
            "Energy change: {}",
            format_energy_joules(energy.total_joules() - initial_total_energy)
        ));
    }
    ui.label(format!(
        "Kinetic energy: {}",
        format_energy_joules(energy.kinetic_joules)
    ));
    ui.label(format!(
        "Potential energy: {}",
        format_energy_joules(energy.potential_joules)
    ));
}

pub fn format_km(km: f64) -> String {
    let absolute_km = km.abs();
    if absolute_km >= 1_000_000.0 {
        format!("{:.2} M km", km / 1_000_000.0)
    } else if absolute_km >= 10_000.0 {
        format!("{km:.0} km")
    } else {
        format!("{km:.1} km")
    }
}

pub fn show_selected_body_window(
    ctx: &egui::Context,
    world: &mut World,
    physics: &NBodySimulation,
    selected_body: Option<Entity>,
    initial_total_energy: Option<f64>,
) {
    let Some(body_entity) = selected_body else {
        return;
    };

    let kind = world.kind(body_entity);
    let body = world.body(body_entity);
    let position = physics.position(body_entity);
    let velocity = physics.velocity(body_entity);

    let speed_au_per_year = velocity.length();
    let speed_km_per_second = speed_au_per_year * AU_PER_YEAR_TO_KM_PER_SECOND;

    let radius_au = body.radius_km as f64 / AU_KM;

    let (sun_entity, sun_distance) = world
        .first_entity_of_kind(CelestialKind::Star)
        .map_or((None, position.length()), |sun| {
            (Some(sun), (position - physics.position(sun)).length())
        });

    let star_brightness = sun_entity
        .and_then(|sun| match world.render(sun).material {
            MaterialComponent::Star(material) => Some(material.brightness as f64),
            _ => None,
        })
        .unwrap_or(1.0);

    let atmosphere_density = world.atmosphere(body_entity).map(|a| a.density as f64);

    let estimated_temperature = if kind == CelestialKind::Planet || kind == CelestialKind::Moon {
        let base = estimate_equilibrium_temperature_k(sun_distance, star_brightness);
        Some(apply_atmosphere_greenhouse(base, atmosphere_density))
    } else {
        None
    };

    let window_frame = egui::Frame::window(ctx.global_style().as_ref());

    let body_kind = match kind {
        CelestialKind::Star => "Star",
        CelestialKind::Planet => "Planet",
        CelestialKind::Moon => "Moon",
    };
    let window_title = format!("{}  ·  {body_kind}", world.name(body_entity));

    egui::Window::new(window_title)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
        .collapsible(false)
        .default_width(360.0)
        .resizable(true)
        .vscroll(true)
        .frame(window_frame)
        .show(ctx, |ui| {
            let mut edited_name = world.name(body_entity).to_owned();

            ui.horizontal(|ui| {
                ui.label("Name:");

                if ui.text_edit_singleline(&mut edited_name).changed() {
                    world.set_name(body_entity, edited_name);
                }
            });

            ui.separator();

            if kind == CelestialKind::Moon {
                if let Some(parent) = world.parent(body_entity).map(|parent| parent.entity) {
                    let parent_position = physics.position(parent);
                    let parent_velocity = physics.velocity(parent);

                    let parent_distance = (position - parent_position).length();
                    let parent_relative_velocity = velocity - parent_velocity;
                    let parent_relative_speed =
                        parent_relative_velocity.length() * AU_PER_YEAR_TO_KM_PER_SECOND;
                    let parent_relative_speed_au_per_year = parent_relative_velocity.length();

                    ui.label(format!("Parent planet: {}", world.name(parent)));
                    ui.label(format!(
                        "Orbital speed around parent: {parent_relative_speed:.2} km/s"
                    ));
                    ui.label(format!(
                        "Parent-relative speed: {parent_relative_speed_au_per_year:.3} AU/year"
                    ));

                    ui.label(format!(
                        "Distance from parent: {parent_distance:.4} AU ({})",
                        format_km(au_to_km(parent_distance))
                    ));

                    ui.separator();
                    ui.label(format!(
                        "Solar-system velocity: {speed_km_per_second:.2} km/s"
                    ));
                    ui.label(format!(
                        "Solar-system speed: {speed_au_per_year:.3} AU/year"
                    ));

                    ui.separator();
                }
            } else {
                ui.label(format!("Velocity: {speed_km_per_second:.2} km/s"));
                ui.label(format!("Orbital speed: {speed_au_per_year:.3} AU/year"));
            }

            ui.label(format!(
                "Distance from the star: {sun_distance:.3} AU ({})",
                format_km(au_to_km(sun_distance))
            ));

            if let Some(estimated_temperature) = estimated_temperature {
                ui.label(match atmosphere_density {
                    Some(density) => format!("Atmosphere density: {density:.2}"),
                    None => "Atmosphere density: None".to_string(),
                });
                ui.label(format!(
                    "Estimated temperature: {}",
                    format_temperature(estimated_temperature)
                ));
            }

            ui.label(format!("Mass: {:.3e} kg", body.mass));

            if let Some(energy) = physics.entity_energy(body_entity) {
                ui.separator();
                ui.label("Energy");
                show_energy_labels(ui, energy, initial_total_energy);
            }

            ui.label(format!(
                "Radius: {} ({radius_au:.8} AU)",
                format_km(body.radius_km as f64)
            ));

            ui.label(format!(
                "Position: x {:.2} AU, y {:.2} AU, z {:.2} AU",
                position.x, position.y, position.z
            ));

            if let Some(orbit) = body.orbit {
                ui.separator();

                ui.label("Orbit");
                ui.label(format!("Semi-major axis: {:.4} AU", orbit.semi_major_axis));
                ui.label(format!("Semi-minor axis: {:.4} AU", orbit.semi_minor_axis));

                ui.label(format!(
                    "Orbit direction: {}",
                    if orbit.angular_speed < 0.0 {
                        "retrograde"
                    } else {
                        "prograde"
                    }
                ));

                ui.label(format!("Inclination: {:.2} rad", orbit.inclination));

                ui.label(format!("Phase: {:.2} rad", orbit.phase));
            }

            if kind != CelestialKind::Planet {
                return;
            }

            ui.separator();
            ui.label("Moons");

            let moons: Vec<Entity> = world
                .children_of_kind(body_entity, CelestialKind::Moon)
                .collect();
            let mut moon_count = 0;

            for moon in moons {
                moon_count += 1;

                let moon_relative_speed =
                    (physics.velocity(moon) - velocity).length() * AU_PER_YEAR_TO_KM_PER_SECOND;

                let moon_distance = (physics.position(moon) - position).length();
                let moon_sun_distance = sun_entity.map_or(physics.position(moon).length(), |sun| {
                    (physics.position(moon) - physics.position(sun)).length()
                });
                let moon_atmosphere_density = world.atmosphere(moon).map(|a| a.density as f64);
                let moon_base_temperature =
                    estimate_equilibrium_temperature_k(moon_sun_distance, star_brightness);
                let moon_temperature =
                    apply_atmosphere_greenhouse(moon_base_temperature, moon_atmosphere_density);

                ui.horizontal(|ui| {
                    let mut moon_name = world.name(moon).to_owned();
                    ui.label("Name:");
                    if ui.text_edit_singleline(&mut moon_name).changed() {
                        world.set_name(moon, moon_name);
                    }
                });
                ui.label(format!(
                    "{:.2} km/s  {:.4} AU ({})  {}",
                    moon_relative_speed,
                    moon_distance,
                    format_km(au_to_km(moon_distance)),
                    format_temperature(moon_temperature),
                ));
                if let Some(moon_energy) = physics.entity_energy(moon) {
                    ui.label(format!(
                        "Energy: {}",
                        format_energy_joules(moon_energy.total_joules())
                    ));
                }
                ui.separator();
            }

            if moon_count == 0 {
                ui.label("None");
            }
        });
}
