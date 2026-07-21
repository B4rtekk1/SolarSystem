use self::draw_indirect::{
    IndexedIndirectArgs, IndirectBatch, append_kind_batches, entity_indirect_offset,
    indexed_indirect_args,
};
use self::metrics::{
    HISTORY_SAMPLE_LIMIT, MetricsSample, initial_total_energy_by_entity, show_metrics_chart,
};
use self::picking::pick_radius;
use self::ui::{
    UI_ACCENT, UI_MUTED, configure_egui, show_body_browser, ui_section_heading,
    world_name_for_label,
};
use crate::camera::Camera;
use crate::constants::{
    DEFAULT_ORBIT_THICKNESS_SCALE, DEFAULT_SIMULATION_SPEED, MAX_ORBIT_THICKNESS_SCALE,
    MAX_SIMULATION_SPEED, MIN_ORBIT_THICKNESS_SCALE, MIN_SIMULATION_SPEED, MSAA_SAMPLE_COUNT,
    OrbitSegment, SPHERE_LATITUDES, SPHERE_LONGITUDES,
};
use crate::ecs::{CelestialKind, Entity, World};
use crate::fps_overlay::FpsOverlay;
use crate::geometry::create_sphere;
use crate::nbody::{NBodyConfig, NBodySimulation};
use crate::orbit_render::{
    build_kepler_orbit_segments, max_orbit_segment_count, orbit_draw_vertex_count,
    orbit_width_scale,
};
use crate::pipeline::{
    create_screen_dim_pipeline, create_sphere_overlay_pipeline, create_sphere_pipeline,
    create_sphere_replace_overlay_pipeline, create_text_overlay_pipeline,
};
use crate::render_utils::{
    DepthTarget, MsaaTarget, alpha_blending_fragment_state, alpha_blending_fragment_targets,
    create_depth_target, create_msaa_target, depth_stencil_state,
    read_only_storage_buffer_layout_entry, uniform_buffer_layout_entry,
};
use crate::ring_particles::PlanetRingSystem;
use crate::save::{SaveData, load_from_file, save_to_file};
use crate::scene::create_world;
use crate::stars::Starfield;
use crate::uniforms::{
    ObjectUniform, entity_object_uniform, ray_sphere_distance, rendered_entity_position,
};
use crate::utils::show_selected_body_window;
use egui_wgpu::{
    Renderer as EguiRenderer, RendererOptions as EguiRendererOptions, ScreenDescriptor,
};
use egui_winit::State as EguiWinitState;
use rfd::FileDialog;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use wgpu::Surface;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;
use winit::window::{Fullscreen, Window};

mod draw_indirect;
mod egui_panel;
mod init;
mod interaction;
mod metrics;
mod persistence;
mod picking;
mod render;
mod ui;

const MIN_WINDOW_CONTROL_WIDTH: u32 = 320;
const MIN_WINDOW_CONTROL_HEIGHT: u32 = 240;
const MAX_WINDOW_CONTROL_WIDTH: u32 = 7680;
const MAX_WINDOW_CONTROL_HEIGHT: u32 = 4320;
const CONTROLS_PANEL_DEFAULT_WIDTH: f32 = 292.0;
const CONTROLS_PANEL_DEFAULT_HEIGHT: f32 = 410.0;
const CONTROLS_PANEL_MIN_WIDTH: f32 = 252.0;
const CONTROLS_PANEL_MIN_HEIGHT: f32 = 120.0;
const DEFAULT_SAVE_PATH: &str = "solar_system.orbs";

fn with_orbs_extension(mut path: PathBuf) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension("orbs");
    }
    path
}

pub struct State {
    pub window: Arc<Window>,
    surface: Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    sun_pipeline: wgpu::RenderPipeline,
    sun_focus_pipeline: wgpu::RenderPipeline,
    starfield: Starfield,
    planet_pipeline: wgpu::RenderPipeline,
    moon_pipeline: wgpu::RenderPipeline,
    planet_focus_pipeline: wgpu::RenderPipeline,
    planet_rings: PlanetRingSystem,
    orbit_pipeline: wgpu::RenderPipeline,
    screen_dim_pipeline: wgpu::RenderPipeline,
    text_overlay_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    orbit_vertex_count: u32,
    orbit_buffer: wgpu::Buffer,
    fps_overlay: FpsOverlay,
    egui_ctx: egui::Context,
    pub egui_winit: EguiWinitState,
    egui_renderer: EguiRenderer,
    orbit_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    object_uniforms: Vec<ObjectUniform>,
    indirect_buffer: wgpu::Buffer,
    star_batches: Vec<IndirectBatch>,
    planet_batches: Vec<IndirectBatch>,
    moon_batches: Vec<IndirectBatch>,
    msaa: MsaaTarget,
    depth: DepthTarget,
    camera: Camera,
    world: World,
    physics: NBodySimulation,
    orbit_segments: Vec<OrbitSegment>,
    last_physics_update: Instant,
    rotation_time: f32,
    fps_frame_count: u32,
    fps_last_update: Instant,
    current_fps: f64,
    simulation_speed: f64,
    simulation_paused: bool,
    orbits_visible: bool,
    planet_orbits_visible: bool,
    moon_orbits_visible: bool,
    orbit_thickness_scale: f32,
    selected_body: Option<Entity>,
    camera_follow_enabled: bool,
    initial_total_energy_by_entity: Vec<Option<f64>>,
    window_width_control: u32,
    window_height_control: u32,
    save_status: Option<String>,
    body_search: String,
    metrics_history: VecDeque<MetricsSample>,
}
