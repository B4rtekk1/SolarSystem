use crate::constants::GOOGLE_SANS_BYTES;
use crate::ecs::{CelestialKind, Entity, World};
use std::sync::Arc;

pub const UI_ACCENT: egui::Color32 = egui::Color32::from_rgb(92, 225, 255);
pub const UI_TEXT: egui::Color32 = egui::Color32::from_rgb(226, 237, 250);
pub const UI_MUTED: egui::Color32 = egui::Color32::from_rgb(139, 160, 186);

pub fn configure_egui(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Google Sans".to_owned(),
        Arc::new(egui::FontData::from_static(GOOGLE_SANS_BYTES)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Google Sans".to_owned());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.window_margin = egui::Margin::symmetric(16, 14);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.slider_width = 168.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(20.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );

    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(UI_TEXT);
    visuals.weak_text_color = Some(UI_MUTED);
    visuals.window_fill = egui::Color32::from_rgba_unmultiplied(7, 14, 30, 242);
    visuals.panel_fill = egui::Color32::from_rgb(7, 14, 30);
    visuals.extreme_bg_color = egui::Color32::from_rgb(4, 10, 24);
    visuals.faint_bg_color = egui::Color32::from_rgb(12, 26, 48);
    visuals.code_bg_color = egui::Color32::from_rgb(12, 26, 48);
    visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 74, 104));
    visuals.window_corner_radius = egui::CornerRadius::same(14);
    visuals.menu_corner_radius = egui::CornerRadius::same(10);
    visuals.window_shadow = egui::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 2,
        color: egui::Color32::from_black_alpha(130),
    };
    visuals.popup_shadow = visuals.window_shadow;
    visuals.selection.bg_fill = egui::Color32::from_rgb(26, 121, 155);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.hyperlink_color = UI_ACCENT;
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 22, 42);
    visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(10, 22, 42);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(27, 55, 80));
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, UI_TEXT);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(13, 29, 52);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(13, 29, 52);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(31, 62, 88));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, UI_TEXT);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(18, 56, 79);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(18, 56, 79);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, UI_ACCENT);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(24, 112, 142);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(24, 112, 142);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, UI_ACCENT);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.open = visuals.widgets.active;

    style.visuals = visuals;
    ctx.set_global_style(style);
}

pub fn ui_section_heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .size(11.0)
            .color(UI_ACCENT)
            .strong(),
    );
    ui.separator();
}

pub fn kind_label(kind: CelestialKind) -> &'static str {
    match kind {
        CelestialKind::Star => "star",
        CelestialKind::Planet => "planet",
        CelestialKind::Moon => "moon",
    }
}

pub fn world_name_for_label(world: &World, entity: Entity) -> String {
    format!(
        "{} ({})",
        world.name(entity),
        kind_label(world.kind(entity))
    )
}

pub fn show_body_browser(
    ui: &mut egui::Ui,
    world: &World,
    search: &str,
    selected_body: Option<Entity>,
) -> Option<Entity> {
    let needle = search.trim().to_lowercase();
    let mut go_to = None;
    let mut shown = 0;

    egui::ScrollArea::vertical()
        .max_height(176.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for entity in world.entities() {
                let name = world.name(entity);
                if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                    continue;
                }
                shown += 1;
                let selected = selected_body == Some(entity);
                ui.horizontal(|ui| {
                    let label = if selected {
                        format!("{} ({})", name, kind_label(world.kind(entity)))
                    } else {
                        name.to_string()
                    };
                    ui.add_sized(
                        [ui.available_width().max(96.0) - 96.0, 24.0],
                        egui::Label::new(label).truncate(),
                    );
                    if ui
                        .add_sized([88.0, 24.0], egui::Button::new("Przejdź do"))
                        .clicked()
                    {
                        go_to = Some(entity);
                    }
                });
            }
        });

    if shown == 0 {
        ui.label(egui::RichText::new("Brak wyników").small().color(UI_MUTED));
    }

    go_to
}
