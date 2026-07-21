use super::*;

impl State {
    pub fn render(&mut self) {
        let now = Instant::now();
        let frame_seconds = now.duration_since(self.last_physics_update).as_secs_f64();
        self.last_physics_update = now;
        let (egui_primitives, egui_textures_delta, egui_screen_descriptor) = self.run_egui();
        if !self.simulation_paused {
            let scaled_frame_seconds = frame_seconds * self.simulation_speed;
            if scaled_frame_seconds.is_finite() && scaled_frame_seconds > 0.0 {
                self.rotation_time += scaled_frame_seconds as f32;
            }
            self.physics
                .advance_scaled(frame_seconds, self.simulation_speed);
            self.record_metrics_sample();
        }
        self.update_camera_follow_target();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let camera_uniform = self
            .camera
            .view_projection(self.config.width, self.config.height);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        if self.orbits_visible {
            self.upload_orbit_segments();
        }

        for entity in self.world.entities() {
            let uniform = entity_object_uniform(
                &self.world,
                &self.physics,
                entity,
                self.rotation_time,
                self.selected_body,
            );
            self.object_uniforms[entity.index()] = uniform;
        }
        self.queue.write_buffer(
            &self.object_buffer,
            0,
            bytemuck::cast_slice(&self.object_uniforms),
        );

        self.planet_rings.update(
            &self.queue,
            &self.world,
            &self.physics,
            self.rotation_time,
            self.selected_body,
        );

        self.fps_overlay.update(
            &self.queue,
            self.current_fps,
            self.config.width,
            self.config.height,
        );

        let mut encoder = self.device.create_command_encoder(&Default::default());
        for (id, image_delta) in &egui_textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        let egui_command_buffers = self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &egui_primitives,
            &egui_screen_descriptor,
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa.view,
                    resolve_target: Some(&view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.003,
                            g: 0.008,
                            b: 0.025,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            self.starfield.render(&mut pass, &self.camera_bind_group);

            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            pass.set_pipeline(&self.sun_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.object_bind_group, &[]);
            for batch in &self.star_batches {
                if batch.instance_count > 0 {
                    pass.draw_indexed_indirect(&self.indirect_buffer, batch.offset);
                }
            }

            if self.orbits_visible && self.orbit_vertex_count > 0 {
                pass.set_pipeline(&self.orbit_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_bind_group(1, &self.orbit_bind_group, &[]);
                pass.draw(0..self.orbit_vertex_count, 0..1);
            }

            pass.set_pipeline(&self.planet_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(1, &self.object_bind_group, &[]);
            for batch in &self.planet_batches {
                if batch.instance_count > 0 {
                    pass.draw_indexed_indirect(&self.indirect_buffer, batch.offset);
                }
            }

            pass.set_pipeline(&self.moon_pipeline);
            pass.set_bind_group(1, &self.object_bind_group, &[]);
            for batch in &self.moon_batches {
                if batch.instance_count > 0 {
                    pass.draw_indexed_indirect(&self.indirect_buffer, batch.offset);
                }
            }

            if let Some(selected_body) = self.selected_body {
                pass.set_pipeline(&self.screen_dim_pipeline);
                pass.draw(0..3, 0..1);

                if self.world.kind(selected_body) == CelestialKind::Star {
                    pass.set_pipeline(&self.sun_focus_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                    pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_bind_group(1, &self.object_bind_group, &[]);
                    pass.draw_indexed_indirect(
                        &self.indirect_buffer,
                        entity_indirect_offset(selected_body),
                    );
                }

                pass.set_pipeline(&self.planet_focus_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_bind_group(1, &self.object_bind_group, &[]);
                for entity in self.world.entities().filter(|entity| {
                    *entity == selected_body
                        || (self.world.kind(selected_body) == CelestialKind::Moon
                            && self.world.kind(*entity) == CelestialKind::Planet
                            && self
                                .world
                                .parent(selected_body)
                                .is_some_and(|parent| parent.entity == *entity))
                }) {
                    pass.draw_indexed_indirect(
                        &self.indirect_buffer,
                        entity_indirect_offset(entity),
                    );
                }

                pass.set_pipeline(&self.moon_pipeline);
                pass.set_bind_group(1, &self.object_bind_group, &[]);
                for entity in self.world.entities().filter(|entity| {
                    *entity == selected_body
                        || (self.world.kind(selected_body) == CelestialKind::Planet
                            && self.world.kind(*entity) == CelestialKind::Moon
                            && self
                                .world
                                .parent(*entity)
                                .is_some_and(|parent| parent.entity == selected_body))
                }) {
                    if self.world.kind(entity) == CelestialKind::Moon {
                        pass.draw_indexed_indirect(
                            &self.indirect_buffer,
                            entity_indirect_offset(entity),
                        );
                    }
                }
            }

            self.planet_rings.render(&mut pass, &self.camera_bind_group);

            if self.fps_overlay.text_vertex_count > 0 {
                pass.set_pipeline(&self.text_overlay_pipeline);
                pass.set_bind_group(0, &self.fps_overlay.text_bind_group, &[]);
                pass.set_vertex_buffer(0, self.fps_overlay.text_vertex_buffer.slice(..));
                pass.draw(0..self.fps_overlay.text_vertex_count, 0..1);
            }
        }

        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.egui_renderer
                .render(&mut pass, &egui_primitives, &egui_screen_descriptor);
        }

        for id in &egui_textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(
            egui_command_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        frame.present();
        self.update_fps_counter(Instant::now());
    }

    pub fn wait_idle(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    pub fn should_auto_redraw(&self) -> bool {
        !self.simulation_paused
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}
