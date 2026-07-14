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
    create_depth_target, create_msaa_target, depth_stencil_state, uniform_buffer_layout_entry,
};
use crate::ring_particles::PlanetRingSystem;
use crate::save::{SaveData, load_from_file, save_to_file};
use crate::scene::create_world;
use crate::stars::Starfield;
use crate::uniforms::{entity_object_uniform, ray_sphere_distance, rendered_entity_position};
use crate::utils::show_selected_body_window;
use egui_wgpu::{
    Renderer as EguiRenderer, RendererOptions as EguiRendererOptions, ScreenDescriptor,
};
use egui_winit::State as EguiWinitState;
use rfd::FileDialog;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use wgpu::Surface;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;
use winit::window::{Fullscreen, Window};

struct ObjectGpu {
    entity: Entity,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
}

const MIN_WINDOW_CONTROL_WIDTH: u32 = 320;
const MIN_WINDOW_CONTROL_HEIGHT: u32 = 240;
const MAX_WINDOW_CONTROL_WIDTH: u32 = 7680;
const MAX_WINDOW_CONTROL_HEIGHT: u32 = 4320;
const CONTROLS_PANEL_DEFAULT_WIDTH: f32 = 300.0;
const CONTROLS_PANEL_DEFAULT_HEIGHT: f32 = 360.0;
const CONTROLS_PANEL_MIN_WIDTH: f32 = 240.0;
const CONTROLS_PANEL_MIN_HEIGHT: f32 = 120.0;
const DEFAULT_SAVE_PATH: &str = "solar_system.orbs";

fn initial_total_energy_by_entity(world: &World, physics: &NBodySimulation) -> Vec<Option<f64>> {
    let mut energies = vec![None; world.entity_capacity()];
    for entity in world.entities() {
        energies[entity.index()] = physics.entity_energy(entity).and_then(|energy| {
            let total = energy.total_joules();
            total.is_finite().then_some(total)
        });
    }
    energies
}

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
    index_count: u32,
    orbit_vertex_count: u32,
    orbit_buffer: wgpu::Buffer,
    fps_overlay: FpsOverlay,
    egui_ctx: egui::Context,
    pub egui_winit: EguiWinitState,
    egui_renderer: EguiRenderer,
    orbit_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    object_gpu: Vec<ObjectGpu>,
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
}

impl State {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let sun_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sun Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sun.wgsl").into()),
        });
        let planet_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Planet Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/planet.wgsl").into()),
        });
        let orbit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Orbit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/orbit.wgsl").into()),
        });
        let text_overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text_overlay.wgsl").into()),
        });
        let screen_dim_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Screen Dim Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/screen_dim.wgsl").into()),
        });

        let camera = Camera::default();
        let camera_uniform = camera.view_projection(config.width, config.height);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[uniform_buffer_layout_entry(wgpu::ShaderStages::VERTEX)],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let object_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Object Bind Group Layout"),
                entries: &[uniform_buffer_layout_entry(
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                )],
            });

        let orbit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Orbit Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let text_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Text Overlay Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let world = create_world();
        let physics = NBodySimulation::from_world(&world, NBodyConfig::default());
        let initial_total_energy_by_entity = initial_total_energy_by_entity(&world, &physics);

        let mut object_gpu = Vec::with_capacity(world.entity_capacity());
        for entity in world.entities() {
            let object_uniform = entity_object_uniform(&world, &physics, entity, 0.0, None);
            let object_name = world.name(entity);
            let object_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{object_name} Object Buffer")),
                contents: bytemuck::cast_slice(&[object_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("{object_name} Object Bind Group")),
                layout: &object_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: object_buffer.as_entire_binding(),
                }],
            });
            object_gpu.push(ObjectGpu {
                entity,
                object_buffer,
                object_bind_group,
            });
        }

        let orbit_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Orbit Storage Buffer"),
            size: max_orbit_segment_count(&world, physics.planet_entities().len()) as u64
                * size_of::<OrbitSegment>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut orbit_segments = Vec::with_capacity(max_orbit_segment_count(
            &world,
            physics.planet_entities().len(),
        ));
        build_kepler_orbit_segments(
            &world,
            &physics,
            physics.planet_entities(),
            orbit_width_scale(&camera),
            true,
            true,
            DEFAULT_ORBIT_THICKNESS_SCALE,
            &mut orbit_segments,
        );
        if !orbit_segments.is_empty() {
            queue.write_buffer(&orbit_buffer, 0, bytemuck::cast_slice(&orbit_segments));
        }
        let orbit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Orbit Bind Group"),
            layout: &orbit_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: orbit_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&object_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let sun_pipeline = create_sphere_pipeline(
            &device,
            format,
            &pipeline_layout,
            &sun_shader,
            MSAA_SAMPLE_COUNT,
            "Sun Pipeline",
        );
        let sun_focus_pipeline = create_sphere_replace_overlay_pipeline(
            &device,
            format,
            &pipeline_layout,
            &sun_shader,
            MSAA_SAMPLE_COUNT,
            "Sun Focus Overlay Pipeline",
        );
        let starfield = Starfield::new(
            &device,
            format,
            MSAA_SAMPLE_COUNT,
            &camera_bind_group_layout,
        );
        let planet_rings = PlanetRingSystem::new(
            &device,
            format,
            MSAA_SAMPLE_COUNT,
            &camera_bind_group_layout,
            &world,
        );
        let planet_pipeline = create_sphere_overlay_pipeline(
            &device,
            format,
            &pipeline_layout,
            &planet_shader,
            MSAA_SAMPLE_COUNT,
            "Planet Overlay Pipeline",
        );
        let moon_pipeline = create_sphere_overlay_pipeline(
            &device,
            format,
            &pipeline_layout,
            &planet_shader,
            MSAA_SAMPLE_COUNT,
            "Moon Overlay Pipeline",
        );
        let planet_focus_pipeline = create_sphere_overlay_pipeline(
            &device,
            format,
            &pipeline_layout,
            &planet_shader,
            MSAA_SAMPLE_COUNT,
            "Planet Focus Overlay Pipeline",
        );
        let screen_dim_pipeline =
            create_screen_dim_pipeline(&device, format, &screen_dim_shader, MSAA_SAMPLE_COUNT);
        let text_overlay_pipeline = create_text_overlay_pipeline(
            &device,
            format,
            &text_overlay_shader,
            MSAA_SAMPLE_COUNT,
            &text_overlay_bind_group_layout,
        );

        let orbit_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Orbit Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&orbit_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let orbit_fragment_targets = alpha_blending_fragment_targets(format);
        let orbit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Orbit Pipeline"),
            layout: Some(&orbit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &orbit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(alpha_blending_fragment_state(
                &orbit_shader,
                &orbit_fragment_targets,
            )),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil_state(false, wgpu::CompareFunction::LessEqual)),
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLE_COUNT,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let (vertices, indices) = create_sphere(SPHERE_LATITUDES, SPHERE_LONGITUDES, 1.0);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let index_count = indices.len() as u32;
        let orbit_vertex_count = orbit_draw_vertex_count(&orbit_segments);
        let fps_overlay = FpsOverlay::new(
            &device,
            &queue,
            &text_overlay_bind_group_layout,
            config.width,
            config.height,
        );
        let msaa = create_msaa_target(&device, config.width, config.height, format);
        let depth = create_depth_target(&device, config.width, config.height);
        let egui_ctx = egui::Context::default();
        let egui_winit = EguiWinitState::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let egui_renderer = EguiRenderer::new(&device, format, EguiRendererOptions::default());
        let now = Instant::now();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            sun_pipeline,
            sun_focus_pipeline,
            starfield,
            planet_pipeline,
            moon_pipeline,
            planet_focus_pipeline,
            planet_rings,
            orbit_pipeline,
            screen_dim_pipeline,
            text_overlay_pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            orbit_vertex_count,
            orbit_buffer,
            fps_overlay,
            egui_ctx,
            egui_winit,
            egui_renderer,
            orbit_bind_group,
            camera_buffer,
            camera_bind_group,
            object_gpu,
            msaa,
            depth,
            camera,
            world,
            physics,
            orbit_segments,
            last_physics_update: now,
            rotation_time: 0.0,
            fps_frame_count: 0,
            fps_last_update: now,
            current_fps: 0.0,
            simulation_speed: DEFAULT_SIMULATION_SPEED,
            simulation_paused: false,
            orbits_visible: true,
            planet_orbits_visible: true,
            moon_orbits_visible: true,
            orbit_thickness_scale: DEFAULT_ORBIT_THICKNESS_SCALE,
            selected_body: None,
            camera_follow_enabled: false,
            initial_total_energy_by_entity,
            window_width_control: size.width,
            window_height_control: size.height,
            save_status: None,
        }
    }

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
                self.save_status = Some(format!("Load failed: {error}"));
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
        if data.world.entity_capacity() != self.object_gpu.len() {
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

        self.last_physics_update = Instant::now();
        Ok(())
    }

    fn request_window_size(&mut self, width: u32, height: u32) {
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
        self.update_camera_follow_target();
        true
    }

    pub fn clear_selected_body(&mut self) -> bool {
        if self.selected_body.is_none() {
            return false;
        }

        self.selected_body = None;
        self.camera_follow_enabled = false;
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

    fn update_camera_follow_target(&mut self) {
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
            let radius = match self.world.kind(entity) {
                CelestialKind::Star => (body.render_radius * 2.0).max(0.5),
                CelestialKind::Planet | CelestialKind::Moon => {
                    (body.render_radius * 1.45).max(0.08)
                }
            };
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

    fn upload_orbit_segments(&mut self) {
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

    fn update_fps_counter(&mut self, now: Instant) {
        self.fps_frame_count += 1;

        let elapsed = now.duration_since(self.fps_last_update);
        if elapsed.as_secs_f64() < 1.0 {
            return;
        }

        self.current_fps = self.fps_frame_count as f64 / elapsed.as_secs_f64();
        self.fps_frame_count = 0;
        self.fps_last_update = now;
    }

    fn run_egui(
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
        let mut apply_window_size = false;
        let mut save_requested = false;
        let mut load_requested = false;
        let mut save_as_requested = false;
        let mut load_file_requested = false;
        let selected_body = self.selected_body;

        let full_output = egui_ctx.run_ui(raw_input, |ui| {
            let window_frame = egui::Frame::window(ui.style().as_ref()).shadow(egui::Shadow::NONE);
            egui::Window::new("Controls")
                .default_pos(egui::pos2(8.0, 8.0))
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
                    let text = if simulation_paused { "Resume" } else { "Pause" };
                    if ui.button(text).clicked() {
                        simulation_paused = !simulation_paused;
                    }
                    ui.add(
                        egui::Slider::new(
                            &mut simulation_speed,
                            MIN_SIMULATION_SPEED..=MAX_SIMULATION_SPEED,
                        )
                        .text("Simulation Speed"),
                    );
                    ui.label(format!("{simulation_speed:.2}x"));
                    ui.separator();
                    ui.heading("Camera");
                    ui.add_enabled_ui(selected_body.is_some(), |ui| {
                        ui.checkbox(&mut camera_follow_enabled, "Follow selected body");
                    });
                    ui.separator();
                    ui.heading("Orbits");
                    ui.checkbox(&mut orbits_visible, "Show orbits");
                    ui.add_enabled_ui(orbits_visible, |ui| {
                        ui.checkbox(&mut planet_orbits_visible, "Show planet orbits");
                        ui.checkbox(&mut moon_orbits_visible, "Show moon orbits");
                        ui.add(
                            egui::Slider::new(
                                &mut orbit_thickness_scale,
                                MIN_ORBIT_THICKNESS_SCALE..=MAX_ORBIT_THICKNESS_SCALE,
                            )
                            .text("Orbit thickness"),
                        );
                    });

                    ui.separator();
                    ui.heading("Window");
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
                    apply_window_size = ui.button("Apply size").clicked();
                    ui.separator();
                    ui.heading("Save");
                    ui.horizontal(|ui| {
                        save_requested = ui.button("Save").clicked();
                        load_requested = ui.button("Load").clicked();
                    });
                    ui.horizontal(|ui| {
                        save_as_requested = ui.button("Save As").clicked();
                        load_file_requested = ui.button("Load File").clicked();
                    });
                    if let Some(status) = &self.save_status {
                        ui.label(status);
                    }
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

        self.upload_orbit_segments();

        for object_gpu in &self.object_gpu {
            let uniform = entity_object_uniform(
                &self.world,
                &self.physics,
                object_gpu.entity,
                self.rotation_time,
                self.selected_body,
            );
            self.queue.write_buffer(
                &object_gpu.object_buffer,
                0,
                bytemuck::cast_slice(&[uniform]),
            );
        }

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
                            r: 0.01,
                            g: 0.01,
                            b: 0.04,
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
            for object_gpu in self
                .object_gpu
                .iter()
                .filter(|object| self.world.kind(object.entity) == CelestialKind::Star)
            {
                if self.selected_body == Some(object_gpu.entity) {
                    continue;
                }
                pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
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
            for object_gpu in self
                .object_gpu
                .iter()
                .filter(|object| self.world.kind(object.entity) == CelestialKind::Planet)
            {
                pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }

            pass.set_pipeline(&self.moon_pipeline);
            for object_gpu in self
                .object_gpu
                .iter()
                .filter(|object| self.world.kind(object.entity) == CelestialKind::Moon)
            {
                pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }

            if let Some(selected_body) = self.selected_body {
                pass.set_pipeline(&self.screen_dim_pipeline);
                pass.draw(0..3, 0..1);

                if self.world.kind(selected_body) == CelestialKind::Star {
                    pass.set_pipeline(&self.sun_focus_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                    pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    for object_gpu in self
                        .object_gpu
                        .iter()
                        .filter(|object| object.entity == selected_body)
                    {
                        pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                        pass.draw_indexed(0..self.index_count, 0, 0..1);
                    }
                }

                pass.set_pipeline(&self.planet_focus_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                for object_gpu in self.object_gpu.iter().filter(|object| {
                    object.entity == selected_body
                        || (self.world.kind(selected_body) == CelestialKind::Moon
                            && self.world.kind(object.entity) == CelestialKind::Planet
                            && self
                                .world
                                .parent(selected_body)
                                .is_some_and(|parent| parent.entity == object.entity))
                }) {
                    pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                    pass.draw_indexed(0..self.index_count, 0, 0..1);
                }

                pass.set_pipeline(&self.moon_pipeline);
                for object_gpu in self.object_gpu.iter().filter(|object| {
                    object.entity == selected_body
                        || (self.world.kind(selected_body) == CelestialKind::Planet
                            && self.world.kind(object.entity) == CelestialKind::Moon
                            && self
                                .world
                                .parent(object.entity)
                                .is_some_and(|parent| parent.entity == selected_body))
                }) {
                    if self.world.kind(object_gpu.entity) == CelestialKind::Moon {
                        pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                        pass.draw_indexed(0..self.index_count, 0, 0..1);
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
}

impl Drop for State {
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}
