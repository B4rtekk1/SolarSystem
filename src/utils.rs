use crate::constants::{AU_KM, AU_PER_YEAR_TO_KM_PER_SECOND};
use crate::ecs::{CelestialKind, Entity, World};
use crate::nbody::NBodySimulation;

pub fn au_to_km(au: f64) -> f64 {
    au * AU_KM
}

pub fn format_km(km: f64) -> String {
    let absolute_km = km.abs();
    if absolute_km >= 1_000_000.0 {
        format!("{:.2} mln km", km / 1_000_000.0)
    } else if absolute_km >= 10_000.0 {
        format!("{km:.0} km")
    } else {
        format!("{km:.1} km")
    }
}

pub fn show_selected_planet_window(
    ctx: &egui::Context,
    world: &World,
    physics: &NBodySimulation,
    selected_planet: Option<Entity>,
) {
    let Some(planet) = selected_planet else {
        return;
    };

    let body = world.body(planet);
    let position = physics.position(planet);
    let velocity = physics.velocity(planet);
    let speed_au_per_year = velocity.length();
    let speed_km_per_second = speed_au_per_year * AU_PER_YEAR_TO_KM_PER_SECOND;
    let radius_au = body.radius_km as f64 / AU_KM;
    let sun_distance = world
        .first_entity_of_kind(CelestialKind::Star)
        .map_or(position.length(), |sun| {
            (position - physics.position(sun)).length()
        });
    let window_frame = egui::Frame::window(ctx.global_style().as_ref()).shadow(egui::Shadow::NONE);

    egui::Window::new("Planet")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
        .collapsible(false)
        .resizable(false)
        .frame(window_frame)
        .show(ctx, |ui| {
            ui.heading(world.name(planet));
            ui.label(format!("Predkosc: {speed_km_per_second:.2} km/s"));
            ui.label(format!("Predkosc orbitalna: {speed_au_per_year:.3} AU/rok"));
            ui.label(format!(
                "Odleglosc od gwiazdy: {sun_distance:.3} AU ({})",
                format_km(au_to_km(sun_distance))
            ));
            ui.label(format!("Masa: {:.3e} kg", body.mass));
            ui.label(format!(
                "Promien: {} ({radius_au:.8} AU)",
                format_km(body.radius_km as f64)
            ));
            ui.label(format!(
                "Pozycja: x {:.2} AU, y {:.2} AU, z {:.2} AU",
                position.x, position.y, position.z
            ));

            ui.separator();
            ui.label("Ksiezyce");
            let mut moon_count = 0;
            for moon in world.children_of_kind(planet, CelestialKind::Moon) {
                moon_count += 1;
                let moon_relative_speed =
                    (physics.velocity(moon) - velocity).length() * AU_PER_YEAR_TO_KM_PER_SECOND;
                let moon_distance = (physics.position(moon) - position).length();
                ui.label(format!(
                    "{}  {:.2} km/s  {:.4} AU ({})",
                    world.name(moon),
                    moon_relative_speed,
                    moon_distance,
                    format_km(au_to_km(moon_distance))
                ));
            }

            if moon_count == 0 {
                ui.label("Brak");
            }
        });
}