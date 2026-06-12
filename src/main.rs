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
mod state;
mod sun;
mod uniforms;
mod utils;
use app::App;
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
