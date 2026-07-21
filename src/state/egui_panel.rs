use super::*;

impl State {
    pub(super) fn run_egui(
        &mut self,
    ) -> (
        Vec<egui::ClippedPrimitive>,
        egui::TexturesDelta,
        ScreenDescriptor,
    ) {
        let raw_input = self.egui_winit.take_egui_input(&self.window);
        let egui_ctx = self.egui_ctx.clone();
        let mut simulation_speed = self.simulation_speed;
        let mut simulation_paused = self.simulation_paused;
        let mut orbits_visible = self.orbits_visible;
        let mut planet_orbits_visible = self.planet_orbits_visible;
        let mut moon_orbits_visible = self.moon_orbits_visible;
        let mut orbit_thickness_scale = self.orbit_thickness_scale;
        let mut camera_follow_enabled = self.camera_follow_enabled;
        let mut window_width_control = self.window_width_control;
        let mut window_height_control = self.window_height_control;
        let mut body_search = self.body_search.clone();
        let mut apply_window_size = false;
        let mut save_requested = false;
        let mut load_requested = false;
        let mut save_as_requested = false;
        let mut load_file_requested = false;
        let mut reset_camera_requested = false;
        let mut top_view_requested = false;
        let mut ecliptic_view_requested = false;
        let mut selected_view_requested = false;
        let mut go_to_body = None;
        let selected_body = self.selected_body;

        let full_output = egui_ctx.run_ui(raw_input, |ui| {
            let window_frame = egui::Frame::window(ui.style().as_ref());
            egui::Window::new("Solar System")
                .default_pos(egui::pos2(16.0, 16.0))
                .collapsible(true)
                .default_size(egui::vec2(
                    CONTROLS_PANEL_DEFAULT_WIDTH,
                    CONTROLS_PANEL_DEFAULT_HEIGHT,
                ))
                .min_size(egui::vec2(
                    CONTROLS_PANEL_MIN_WIDTH,
                    CONTROLS_PANEL_MIN_HEIGHT,
                ))
                .resizable(true)
                .vscroll(true)
                .frame(window_frame)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        egui::RichText::new("INTERACTIVE ORRERY")
                            .size(11.0)
                            .color(UI_ACCENT)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Explore the Solar System")
                            .size(20.0)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.horizontal(|ui| {
                        let (status, color) = if simulation_paused {
                            ("Simulation paused", egui::Color32::from_rgb(255, 191, 92))
                        } else {
                            ("Simulation running", egui::Color32::from_rgb(91, 232, 174))
                        };
                        ui.colored_label(color, "●");
                        ui.label(egui::RichText::new(status).small().color(UI_MUTED));
                    });
                    ui.add_space(6.0);

                    let text = if simulation_paused {
                        "Resume simulation"
                    } else {
                        "Pause simulation"
                    };
                    if ui
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(egui::RichText::new(text).strong()),
                        )
                        .clicked()
                    {
                        simulation_paused = !simulation_paused;
                    }

                    ui_section_heading(ui, "Time");
                    ui.label("Simulation speed");
                    ui.add(
                        egui::Slider::new(
                            &mut simulation_speed,
                            MIN_SIMULATION_SPEED..=MAX_SIMULATION_SPEED,
                        )
                        .logarithmic(true),
                    );
                    ui.label(
                        egui::RichText::new(format!("{simulation_speed:.2}× time scale"))
                            .small()
                            .color(UI_MUTED),
                    );

                    ui_section_heading(ui, "Camera");
                    ui.add_enabled_ui(selected_body.is_some(), |ui| {
                        ui.checkbox(&mut camera_follow_enabled, "Follow selected body");
                    });
                    ui.columns(2, |columns| {
                        reset_camera_requested = columns[0]
                            .add_sized(
                                [columns[0].available_width(), 30.0],
                                egui::Button::new("Reset kamery"),
                            )
                            .clicked();
                        selected_view_requested = columns[1]
                            .add_enabled(
                                selected_body.is_some(),
                                egui::Button::new("Wybrana planeta"),
                            )
                            .clicked();
                    });
                    ui.columns(2, |columns| {
                        top_view_requested = columns[0]
                            .add_sized(
                                [columns[0].available_width(), 30.0],
                                egui::Button::new("Z góry"),
                            )
                            .clicked();
                        ecliptic_view_requested = columns[1]
                            .add_sized(
                                [columns[1].available_width(), 30.0],
                                egui::Button::new("Ekliptyka"),
                            )
                            .clicked();
                    });
                    ui.label(
                        egui::RichText::new("Drag to pan · right-drag to orbit · scroll to zoom")
                            .small()
                            .color(UI_MUTED),
                    );

                    ui_section_heading(ui, "Objects");
                    ui.add(
                        egui::TextEdit::singleline(&mut body_search)
                            .hint_text("Szukaj obiektu")
                            .desired_width(ui.available_width()),
                    );
                    go_to_body = show_body_browser(ui, &self.world, &body_search, selected_body);

                    ui_section_heading(ui, "Orbit paths");
                    ui.checkbox(&mut orbits_visible, "Show orbits");
                    ui.add_enabled_ui(orbits_visible, |ui| {
                        ui.checkbox(&mut planet_orbits_visible, "Planet paths");
                        ui.checkbox(&mut moon_orbits_visible, "Moon paths");
                        ui.label("Orbit thickness");
                        ui.add(egui::Slider::new(
                            &mut orbit_thickness_scale,
                            MIN_ORBIT_THICKNESS_SCALE..=MAX_ORBIT_THICKNESS_SCALE,
                        ));
                    });

                    ui_section_heading(ui, "Viewport");
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        ui.add(
                            egui::DragValue::new(&mut window_width_control)
                                .range(MIN_WINDOW_CONTROL_WIDTH..=MAX_WINDOW_CONTROL_WIDTH)
                                .speed(16)
                                .suffix(" px"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height");
                        ui.add(
                            egui::DragValue::new(&mut window_height_control)
                                .range(MIN_WINDOW_CONTROL_HEIGHT..=MAX_WINDOW_CONTROL_HEIGHT)
                                .speed(16)
                                .suffix(" px"),
                        );
                    });
                    apply_window_size = ui
                        .add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new("Apply size"),
                        )
                        .clicked();

                    ui_section_heading(ui, "Scene file");
                    ui.columns(2, |columns| {
                        save_requested = columns[0]
                            .add_sized(
                                [columns[0].available_width(), 30.0],
                                egui::Button::new("Save"),
                            )
                            .clicked();
                        load_requested = columns[1]
                            .add_sized(
                                [columns[1].available_width(), 30.0],
                                egui::Button::new("Load"),
                            )
                            .clicked();
                    });
                    ui.columns(2, |columns| {
                        save_as_requested = columns[0]
                            .add_sized(
                                [columns[0].available_width(), 30.0],
                                egui::Button::new("Save as…"),
                            )
                            .clicked();
                        load_file_requested = columns[1]
                            .add_sized(
                                [columns[1].available_width(), 30.0],
                                egui::Button::new("Open file…"),
                            )
                            .clicked();
                    });
                    if let Some(status) = &self.save_status {
                        ui.label(egui::RichText::new(status).small().color(UI_MUTED));
                    }

                    ui_section_heading(ui, "Charts");
                    let energy_values = self
                        .metrics_history
                        .iter()
                        .map(|sample| sample.energy_joules)
                        .collect::<Vec<_>>();
                    let distance_values = self
                        .metrics_history
                        .iter()
                        .map(|sample| sample.distance_au)
                        .collect::<Vec<_>>();
                    let chart_target = selected_body
                        .map(|entity| world_name_for_label(&self.world, entity))
                        .unwrap_or_else(|| "system".to_string());
                    ui.label(
                        egui::RichText::new(format!("Target: {chart_target}"))
                            .small()
                            .color(UI_MUTED),
                    );
                    show_metrics_chart(ui, "Energia", "J", &energy_values);
                    show_metrics_chart(ui, "Odległość", "AU", &distance_values);

                    ui.add_space(6.0);
                    ui.separator();
                    ui.label(
                        egui::RichText::new(
                            "F5 save  ·  F9 load  ·  F11 fullscreen  ·  Esc clear selection",
                        )
                        .small()
                        .color(UI_MUTED),
                    );
                });

            let selected_initial_total_energy = selected_body
                .and_then(|entity| self.initial_total_energy_by_entity.get(entity.index()))
                .and_then(|energy| *energy);
            show_selected_body_window(
                ui.ctx(),
                &mut self.world,
                &self.physics,
                selected_body,
                selected_initial_total_energy,
            );
        });

        self.simulation_speed = simulation_speed.clamp(MIN_SIMULATION_SPEED, MAX_SIMULATION_SPEED);
        self.simulation_paused = simulation_paused;
        self.orbits_visible = orbits_visible;
        self.planet_orbits_visible = planet_orbits_visible;
        self.moon_orbits_visible = moon_orbits_visible;
        self.orbit_thickness_scale =
            orbit_thickness_scale.clamp(MIN_ORBIT_THICKNESS_SCALE, MAX_ORBIT_THICKNESS_SCALE);
        self.camera_follow_enabled = camera_follow_enabled && self.selected_body.is_some();
        self.update_camera_follow_target();
        self.window_width_control = window_width_control;
        self.window_height_control = window_height_control;
        self.body_search = body_search;
        if reset_camera_requested {
            self.reset_camera();
        }
        if top_view_requested {
            self.set_top_view();
        }
        if ecliptic_view_requested {
            self.set_ecliptic_view();
        }
        if selected_view_requested {
            self.focus_selected_planet();
        }
        if let Some(entity) = go_to_body {
            self.go_to_body(entity);
        }
        if apply_window_size {
            self.request_window_size(window_width_control, window_height_control);
        }
        if save_requested {
            self.save_default();
        }
        if load_requested {
            self.load_default();
        }
        if save_as_requested {
            self.save_as_dialog();
        }
        if load_file_requested {
            self.load_dialog();
        }

        let egui::FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            ..
        } = full_output;

        self.egui_winit
            .handle_platform_output(&self.window, platform_output);
        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };

        (clipped_primitives, textures_delta, screen_descriptor)
    }
}
