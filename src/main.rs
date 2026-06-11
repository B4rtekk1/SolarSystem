mod app;
mod camera;
mod color;
mod constants;
mod ecs;
mod fps_overlay;
mod geometry;
mod nbody;
mod orbit;
mod orbit_render;
mod pipeline;
mod render_utils;
mod scene;
mod uniforms;
mod utils;
mod state;
mod sun;

use wgpu::{Surface, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    event_loop::{EventLoop},
};
use app::App;


fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
