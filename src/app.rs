use crate::constants::CLICK_SELECTION_MAX_DRAG_PIXELS;
use crate::state::State;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowAttributes;

#[derive(Default)]
pub struct App {
    state: Option<State>,
    closing: bool,
    rotating_world: bool,
    panning_map: bool,
    cursor_position: Option<(f64, f64)>,
    left_press_cursor: Option<(f64, f64)>,
    left_drag_moved: bool,
    last_cursor: Option<(f64, f64)>,
}

impl App {
    fn shutdown(&mut self) {
        self.closing = true;
        if let Some(state) = self.state.take() {
            state.wait_idle();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default().with_title("Solar WGPU"))
                .unwrap(),
        );

        self.state = Some(pollster::block_on(State::new(window)));
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        let egui_response = state.egui_winit.on_window_event(&state.window, &event);
        if egui_response.repaint {
            state.window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => {
                self.shutdown();
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
            }

            WindowEvent::KeyboardInput { event, .. }
                if !egui_response.consumed
                    && event.state == ElementState::Pressed
                    && !event.repeat =>
            {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if key == KeyCode::F11 {
                        state.toggle_borderless_fullscreen();
                    } else if state.handle_shader_key(key) {
                        state.window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } if !egui_response.consumed => {
                let scroll_delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.02,
                };
                state.zoom_camera(scroll_delta);
                state.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Right,
                ..
            } if !egui_response.consumed => {
                self.rotating_world = button_state == ElementState::Pressed;
                self.last_cursor = None;
            }

            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Left,
                ..
            } => {
                if egui_response.consumed {
                    if button_state == ElementState::Released {
                        self.panning_map = false;
                        self.left_press_cursor = None;
                        self.left_drag_moved = false;
                        self.last_cursor = None;
                    }
                } else {
                    match button_state {
                        ElementState::Pressed => {
                            self.panning_map = true;
                            self.left_press_cursor = self.cursor_position;
                            self.left_drag_moved = false;
                            self.last_cursor = self.cursor_position;
                        }
                        ElementState::Released => {
                            let click_cursor = self.cursor_position.or(self.left_press_cursor);
                            let should_select = !self.left_drag_moved;

                            self.panning_map = false;
                            self.left_press_cursor = None;
                            self.left_drag_moved = false;
                            self.last_cursor = None;

                            if should_select {
                                if let Some(cursor) = click_cursor {
                                    if state.select_body_at(cursor) {
                                        state.window.request_redraw();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } if egui_response.consumed => {
                self.cursor_position = Some((position.x, position.y));
                self.last_cursor = None;
            }

            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x, position.y);
                self.cursor_position = Some(current);
                if let Some((start_x, start_y)) = self.left_press_cursor {
                    let delta_x = current.0 - start_x;
                    let delta_y = current.1 - start_y;
                    if delta_x * delta_x + delta_y * delta_y
                        > CLICK_SELECTION_MAX_DRAG_PIXELS * CLICK_SELECTION_MAX_DRAG_PIXELS
                    {
                        self.left_drag_moved = true;
                    }
                }

                if self.panning_map {
                    if self.left_drag_moved {
                        if let Some((last_x, last_y)) = self.last_cursor {
                            state.pan_camera(current.0 - last_x, current.1 - last_y);
                            state.window.request_redraw();
                        }
                    }
                } else if self.rotating_world {
                    if let Some((last_x, last_y)) = self.last_cursor {
                        state.orbit_camera(current.0 - last_x, current.1 - last_y);
                    }
                    state.window.request_redraw();
                }
                self.last_cursor = Some(current);
            }

            WindowEvent::RedrawRequested if !self.closing => {
                state.render();
                if !self.closing {
                    state.window.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.shutdown();
    }
}
