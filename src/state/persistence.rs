use super::*;

impl State {
    pub fn save_default(&mut self) -> bool {
        match self.save_to_path(DEFAULT_SAVE_PATH) {
            Ok(()) => {
                self.save_status = Some(format!("Saved to {DEFAULT_SAVE_PATH}"));
                true
            }
            Err(error) => {
                self.save_status = Some(format!("Save failed: {error}"));
                false
            }
        }
    }

    pub fn load_default(&mut self) -> bool {
        match self.load_from_path(DEFAULT_SAVE_PATH) {
            Ok(()) => {
                self.save_status = Some(format!("Loaded from {DEFAULT_SAVE_PATH}"));
                true
            }
            Err(error) => {
                self.save_status = Some(format!("Load failed; current session kept: {error}"));
                false
            }
        }
    }

    pub fn save_as_dialog(&mut self) -> bool {
        let Some(path) = FileDialog::new()
            .add_filter("ORBS save", &["orbs"])
            .set_file_name(DEFAULT_SAVE_PATH)
            .save_file()
        else {
            self.save_status = Some("Save canceled".to_string());
            return false;
        };

        let path = with_orbs_extension(path);
        match self.save_to_path(&path) {
            Ok(()) => {
                self.save_status = Some(format!("Saved to {}", path.display()));
                true
            }
            Err(error) => {
                self.save_status = Some(format!("Save failed: {error}"));
                false
            }
        }
    }

    pub fn load_dialog(&mut self) -> bool {
        let Some(path) = FileDialog::new()
            .add_filter("ORBS save", &["orbs"])
            .pick_file()
        else {
            self.save_status = Some("Load canceled".to_string());
            return false;
        };

        match self.load_from_path(&path) {
            Ok(()) => {
                self.save_status = Some(format!("Loaded from {}", path.display()));
                true
            }
            Err(error) => {
                self.save_status = Some(format!("Load failed: {error}"));
                false
            }
        }
    }

    fn save_to_path(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        save_to_file(
            path,
            &SaveData {
                world: self.world.clone(),
                physics: self.physics.clone(),
                camera: self.camera.clone(),
                simulation_speed: self.simulation_speed,
                simulation_paused: self.simulation_paused,
                orbits_visible: self.orbits_visible,
                planet_orbits_visible: self.planet_orbits_visible,
                moon_orbits_visible: self.moon_orbits_visible,
                orbit_thickness_scale: self.orbit_thickness_scale,
                selected_body: self.selected_body,
                camera_follow_enabled: self.camera_follow_enabled,
                initial_total_energy_by_entity: self.initial_total_energy_by_entity.clone(),
                rotation_time: self.rotation_time,
                window_width: self.window_width_control,
                window_height: self.window_height_control,
            },
        )
    }

    fn load_from_path(&mut self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let data = load_from_file(path)?;
        if data.world.entity_capacity() != self.object_uniforms.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Saved world is incompatible with the current renderer",
            ));
        }

        self.world = data.world;
        self.physics = data.physics;
        self.physics.reset_visual_pacing_to_defaults();
        self.camera = data.camera;
        self.simulation_speed = data
            .simulation_speed
            .clamp(MIN_SIMULATION_SPEED, MAX_SIMULATION_SPEED);
        self.simulation_paused = data.simulation_paused;
        self.orbits_visible = data.orbits_visible;
        self.planet_orbits_visible = data.planet_orbits_visible;
        self.moon_orbits_visible = data.moon_orbits_visible;
        self.orbit_thickness_scale = data
            .orbit_thickness_scale
            .clamp(MIN_ORBIT_THICKNESS_SCALE, MAX_ORBIT_THICKNESS_SCALE);
        self.selected_body = data
            .selected_body
            .filter(|entity| entity.index() < self.world.entity_capacity());
        self.camera_follow_enabled = data.camera_follow_enabled && self.selected_body.is_some();
        self.initial_total_energy_by_entity = data.initial_total_energy_by_entity;
        if self.initial_total_energy_by_entity.len() != self.world.entity_capacity() {
            self.initial_total_energy_by_entity =
                initial_total_energy_by_entity(&self.world, &self.physics);
        }
        self.rotation_time = data.rotation_time;
        self.window_width_control = data
            .window_width
            .clamp(MIN_WINDOW_CONTROL_WIDTH, MAX_WINDOW_CONTROL_WIDTH);
        self.window_height_control = data
            .window_height
            .clamp(MIN_WINDOW_CONTROL_HEIGHT, MAX_WINDOW_CONTROL_HEIGHT);
        self.metrics_history.clear();

        self.last_physics_update = Instant::now();
        Ok(())
    }
}
