use crate::ecs::World;
use crate::nbody::NBodySimulation;
use crate::state::ui::{UI_ACCENT, UI_MUTED, UI_TEXT};

pub const HISTORY_SAMPLE_LIMIT: usize = 360;

#[derive(Clone, Copy)]
pub struct MetricsSample {
    pub energy_joules: f64,
    pub distance_au: f64,
}

pub fn initial_total_energy_by_entity(
    world: &World,
    physics: &NBodySimulation,
) -> Vec<Option<f64>> {
    let mut energies = vec![None; world.entity_capacity()];
    for entity in world.entities() {
        energies[entity.index()] = physics.entity_energy(entity).and_then(|energy| {
            let total = energy.total_joules();
            total.is_finite().then_some(total)
        });
    }
    energies
}

pub fn show_metrics_chart(ui: &mut egui::Ui, title: &str, unit: &str, values: &[f64]) {
    ui.label(egui::RichText::new(title).small().color(UI_MUTED));
    let desired_size = egui::vec2(ui.available_width(), 72.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(6, 14, 28));
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(31, 62, 88)),
        egui::StrokeKind::Inside,
    );

    let values: Vec<f64> = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if values.len() < 2 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Zbieranie danych",
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            UI_MUTED,
        );
        return;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).abs().max(f64::EPSILON);
    let to_point = |index: usize, value: f64| {
        let x = rect.left() + index as f32 / (values.len() - 1) as f32 * rect.width();
        let y = rect.bottom() - ((value - min) / span) as f32 * rect.height();
        egui::pos2(x, y)
    };

    for index in 1..values.len() {
        painter.line_segment(
            [
                to_point(index - 1, values[index - 1]),
                to_point(index, values[index]),
            ],
            egui::Stroke::new(1.6, UI_ACCENT),
        );
    }

    if let Some(last) = values.last() {
        painter.text(
            rect.right_top() + egui::vec2(-6.0, 6.0),
            egui::Align2::RIGHT_TOP,
            format!("{last:.3e} {unit}"),
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
            UI_TEXT,
        );
    }
}
