use super::*;

impl State {
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.msaa = create_msaa_target(&self.device, width, height, self.config.format);
        self.depth = create_depth_target(&self.device, width, height);
        self.window_width_control = width;
        self.window_height_control = height;
    }

    pub(super) fn request_window_size(&mut self, width: u32, height: u32) {
        let width = width.clamp(MIN_WINDOW_CONTROL_WIDTH, MAX_WINDOW_CONTROL_WIDTH);
        let height = height.clamp(MIN_WINDOW_CONTROL_HEIGHT, MAX_WINDOW_CONTROL_HEIGHT);
        self.window_width_control = width;
        self.window_height_control = height;

        let requested_size = PhysicalSize::new(width, height);
        if let Some(applied_size) = self.window.request_inner_size(requested_size) {
            self.resize(applied_size.width, applied_size.height);
        }
    }

    pub fn orbit_camera(&mut self, delta_x: f64, delta_y: f64) {
        self.camera.orbit(delta_x, delta_y);
    }

    pub fn pan_camera(&mut self, delta_x: f64, delta_y: f64) {
        self.camera_follow_enabled = false;
        self.camera.pan(delta_x, delta_y, self.config.height);
    }

    pub fn zoom_camera(&mut self, scroll_delta: f32) {
        self.camera.zoom(scroll_delta);
    }

    pub fn select_body_at(&mut self, cursor: (f64, f64)) -> bool {
        let selected_body = self.pick_body(cursor);
        if self.selected_body == selected_body {
            return false;
        }

        self.selected_body = selected_body;
        self.metrics_history.clear();
        self.update_camera_follow_target();
        true
    }

    pub fn clear_selected_body(&mut self) -> bool {
        if self.selected_body.is_none() {
            return false;
        }

        self.selected_body = None;
        self.camera_follow_enabled = false;
        self.metrics_history.clear();
        true
    }

    pub fn toggle_camera_follow(&mut self) -> bool {
        if self.selected_body.is_none() {
            return false;
        }

        self.camera_follow_enabled = !self.camera_follow_enabled;
        self.update_camera_follow_target();
        true
    }

    pub(super) fn reset_camera(&mut self) {
        self.camera.reset();
        self.camera_follow_enabled = false;
    }

    pub(super) fn set_top_view(&mut self) {
        self.camera.set_top_view();
        self.camera_follow_enabled = false;
    }

    pub(super) fn set_ecliptic_view(&mut self) {
        self.camera.set_ecliptic_view();
        self.camera_follow_enabled = false;
    }

    pub(super) fn go_to_body(&mut self, entity: Entity) {
        if self.selected_body != Some(entity) {
            self.metrics_history.clear();
        }
        self.selected_body = Some(entity);
        self.focus_camera_on(entity);
    }

    pub(super) fn focus_selected_planet(&mut self) {
        if let Some(entity) = self.selected_body {
            let target = if self.world.kind(entity) == CelestialKind::Moon {
                self.world
                    .parent(entity)
                    .map_or(entity, |parent| parent.entity)
            } else {
                entity
            };
            self.focus_camera_on(target);
        }
    }

    fn focus_camera_on(&mut self, entity: Entity) {
        let position = rendered_entity_position(&self.world, &self.physics, entity);
        let distance = (self.world.body(entity).render_radius * 18.0).clamp(1.8, 14.0);
        self.camera.focus_on(position, distance);
        self.camera_follow_enabled = true;
    }

    pub(super) fn record_metrics_sample(&mut self) {
        let energy_joules = self
            .selected_body
            .and_then(|entity| self.physics.entity_energy(entity))
            .unwrap_or_else(|| self.physics.energy())
            .total_joules();
        let distance_au = self.selected_body.map_or(0.0, |entity| {
            let reference = if self.world.kind(entity) == CelestialKind::Moon {
                self.world
                    .parent(entity)
                    .map(|parent| parent.entity)
                    .or_else(|| self.world.first_entity_of_kind(CelestialKind::Star))
            } else {
                self.world.first_entity_of_kind(CelestialKind::Star)
            };
            reference.map_or(0.0, |reference| {
                (self.physics.position(entity) - self.physics.position(reference)).length()
            })
        });

        if energy_joules.is_finite() && distance_au.is_finite() {
            self.metrics_history.push_back(MetricsSample {
                energy_joules,
                distance_au,
            });
            while self.metrics_history.len() > HISTORY_SAMPLE_LIMIT {
                self.metrics_history.pop_front();
            }
        }
    }

    pub(super) fn update_camera_follow_target(&mut self) {
        if !self.camera_follow_enabled {
            return;
        }

        let Some(selected_body) = self.selected_body else {
            self.camera_follow_enabled = false;
            return;
        };

        self.camera.set_target(rendered_entity_position(
            &self.world,
            &self.physics,
            selected_body,
        ));
    }

    fn pick_body(&self, cursor: (f64, f64)) -> Option<Entity> {
        let (ray_origin, ray_direction) = self.camera.screen_ray(
            cursor.0 as f32,
            cursor.1 as f32,
            self.config.width,
            self.config.height,
        );
        let mut closest = None;

        for entity in self.world.entities().filter(|entity| {
            matches!(
                self.world.kind(*entity),
                CelestialKind::Star | CelestialKind::Planet | CelestialKind::Moon
            )
        }) {
            let center = rendered_entity_position(&self.world, &self.physics, entity);
            let body = self.world.body(entity);
            let radius = pick_radius(self.world.kind(entity), body.render_radius);
            let Some(distance) = ray_sphere_distance(ray_origin, ray_direction, center, radius)
            else {
                continue;
            };

            if match closest {
                Some((_, closest_distance)) => distance < closest_distance,
                None => true,
            } {
                closest = Some((entity, distance));
            }
        }

        closest.map(|(entity, _)| entity)
    }

    pub(super) fn upload_orbit_segments(&mut self) {
        self.orbit_segments.clear();
        build_kepler_orbit_segments(
            &self.world,
            &self.physics,
            self.physics.planet_entities(),
            orbit_width_scale(&self.camera),
            self.orbits_visible && self.planet_orbits_visible,
            self.orbits_visible && self.moon_orbits_visible,
            self.orbit_thickness_scale,
            &mut self.orbit_segments,
        );
        self.orbit_vertex_count = orbit_draw_vertex_count(&self.orbit_segments);
        if !self.orbit_segments.is_empty() {
            self.queue.write_buffer(
                &self.orbit_buffer,
                0,
                bytemuck::cast_slice(&self.orbit_segments),
            );
        }
    }

    pub fn toggle_borderless_fullscreen(&self) {
        let fullscreen = if self.window.fullscreen().is_some() {
            None
        } else {
            Some(Fullscreen::Borderless(None))
        };

        self.window.set_fullscreen(fullscreen);
    }

    pub fn handle_shader_key(&mut self, key: KeyCode) -> bool {
        let first_planet = self.world.first_entity_of_kind(CelestialKind::Planet);
        let first_star = self.world.first_entity_of_kind(CelestialKind::Star);

        match key {
            KeyCode::KeyQ => {
                let Some(material) =
                    first_planet.and_then(|entity| self.world.surface_material_mut(entity))
                else {
                    return false;
                };
                material.roughness += 0.05;
            }
            KeyCode::KeyA => {
                let Some(material) =
                    first_planet.and_then(|entity| self.world.surface_material_mut(entity))
                else {
                    return false;
                };
                material.roughness -= 0.05;
            }
            KeyCode::KeyW => {
                let Some(material) =
                    first_planet.and_then(|entity| self.world.surface_material_mut(entity))
                else {
                    return false;
                };
                material.metallic += 0.05;
            }
            KeyCode::KeyS => {
                let Some(material) =
                    first_planet.and_then(|entity| self.world.surface_material_mut(entity))
                else {
                    return false;
                };
                material.metallic -= 0.05;
            }
            KeyCode::KeyE => {
                let Some(atmosphere) =
                    first_planet.and_then(|entity| self.world.atmosphere_mut(entity))
                else {
                    return false;
                };
                atmosphere.density += 0.05;
            }
            KeyCode::KeyD => {
                let Some(atmosphere) =
                    first_planet.and_then(|entity| self.world.atmosphere_mut(entity))
                else {
                    return false;
                };
                atmosphere.density -= 0.05;
            }
            KeyCode::KeyR => {
                let Some(material) =
                    first_star.and_then(|entity| self.world.star_material_mut(entity))
                else {
                    return false;
                };
                material.brightness += 0.1;
            }
            KeyCode::KeyF => {
                let Some(material) =
                    first_star.and_then(|entity| self.world.star_material_mut(entity))
                else {
                    return false;
                };
                material.brightness -= 0.1;
            }
            KeyCode::KeyT => {
                let Some(material) =
                    first_star.and_then(|entity| self.world.star_material_mut(entity))
                else {
                    return false;
                };
                material.surface_temperature += 250.0;
            }
            KeyCode::KeyG => {
                let Some(material) =
                    first_star.and_then(|entity| self.world.star_material_mut(entity))
                else {
                    return false;
                };
                material.surface_temperature -= 250.0;
            }
            _ => return false,
        }

        if let Some(material) =
            first_planet.and_then(|entity| self.world.surface_material_mut(entity))
        {
            material.roughness = material.roughness.clamp(0.0, 1.0);
            material.metallic = material.metallic.clamp(0.0, 1.0);
        }
        if let Some(atmosphere) = first_planet.and_then(|entity| self.world.atmosphere_mut(entity))
        {
            atmosphere.density = atmosphere.density.clamp(0.0, 1.5);
        }
        if let Some(material) = first_star.and_then(|entity| self.world.star_material_mut(entity)) {
            material.brightness = material.brightness.clamp(0.1, 4.0);
            material.surface_temperature = material.surface_temperature.clamp(2500.0, 12000.0);
        }
        true
    }

    pub(super) fn update_fps_counter(&mut self, now: Instant) {
        self.fps_frame_count += 1;

        let elapsed = now.duration_since(self.fps_last_update);
        if elapsed.as_secs_f64() < 1.0 {
            return;
        }

        self.current_fps = self.fps_frame_count as f64 / elapsed.as_secs_f64();
        self.fps_frame_count = 0;
        self.fps_last_update = now;
    }
}
