mod camera;
mod color;
mod ecs;
mod nbody;
mod orbit;
mod fps_overlay;
mod orbit_render;

use std::{
    collections::VecDeque,
    f32::consts::{PI, TAU},
    sync::{Arc},
    time::Instant,
};

use camera::Camera;
use color::Color;
use cosmic_text::{
    Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Shaping,
    SwashCache,
};
use ecs::{
    AtmosphereComponent, BodyComponent, CelestialKind, Entity, MaterialComponent, ObjectBundle,
    RenderComponent, RotationComponent, StarMaterial, SurfaceMaterial, World,
};

use fps_overlay::*;
use orbit_render::*;
use glam::{DVec3, Mat4, Vec3};
use nbody::{NBodyConfig, NBodySimulation};
use orbit::Orbit as PlanetOrbit;
use wgpu::{Surface, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window, WindowAttributes},
};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const SPHERE_LATITUDES: u32 = 48;
const SPHERE_LONGITUDES: u32 = 96;
const ORBIT_TRAIL_POINTS: usize = 768;
const ORBIT_TRAIL_SAMPLE_YEARS: f64 = 1.0 / 96.0;
const ORBIT_FORECAST_MAX_POINTS: usize = 2048;
const ORBIT_FORECAST_SAMPLE_YEARS: f64 = 1.0 / 64.0;
const ORBIT_FORECAST_UPDATE_INTERVAL_SECONDS: f64 = 0.25;
const ORBIT_VERTICES_PER_SEGMENT: usize = 6;
const PLANET_ORBIT_HALF_WIDTH_PIXELS: f32 = 1.75;
const MOON_ORBIT_HALF_WIDTH_PIXELS: f32 = 0.55;
const MOON_ORBIT_FORECAST_MAX_POINTS: usize = 512;
const MOON_ORBIT_FORECAST_SAMPLE_YEARS: f64 = 1.0 / 512.0;
const ORBIT_SPEED: f32 = 0.01;
const PAN_SPEED: f32 = 1.0;
const ZOOM_SPEED: f32 = 0.12;
const MIN_CAMERA_DISTANCE: f32 = 1.35;
const DEFAULT_CAMERA_DISTANCE: f32 = 8.5;
const MAX_CAMERA_DISTANCE: f32 = 40.0;
const MIN_ORBIT_WIDTH_SCALE: f32 = 0.45;
const MAX_ORBIT_WIDTH_SCALE: f32 = 2.4;
const MSAA_SAMPLE_COUNT: u32 = 4;
const EARTH_MASS_KG: f32 = 5.972e24;
const LUNAR_MASS_KG: f32 = 7.342e22;
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
const TEXT_OVERLAY_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];
const TEXT_OVERLAY_MAX_VERTICES: usize = 6;
const FPS_TEXT_TEXTURE_WIDTH: u32 = 152;
const FPS_TEXT_TEXTURE_HEIGHT: u32 = 40;
const FPS_FONT_SIZE: f32 = 23.0;
const FPS_LINE_HEIGHT: f32 = 32.0;
const FPS_OVERLAY_MARGIN: f32 = 6.0;
const GOOGLE_SANS_BYTES: &[u8] =
    include_bytes!("../assets/Google_Sans/GoogleSans-VariableFont_GRAD,opsz,wght.ttf");

type Vertex = [f32; 3];
type TextOverlayVertex = [f32; 4];
type CameraUniform = [f32; 20];
type ObjectUniform = [f32; 32];
type OrbitSegment = [f32; 16];

struct DepthTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct MsaaTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct ObjectGpu {
    entity: Entity,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
}
struct State {
    window: Arc<Window>,
    surface: Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    sun_pipeline: wgpu::RenderPipeline,
    planet_pipeline: wgpu::RenderPipeline,
    orbit_pipeline: wgpu::RenderPipeline,
    text_overlay_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    orbit_vertex_count: u32,
    orbit_buffer: wgpu::Buffer,
    fps_overlay: FpsOverlay,
    orbit_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    object_gpu: Vec<ObjectGpu>,
    msaa: MsaaTarget,
    depth: DepthTarget,
    camera: Camera,
    world: World,
    physics: NBodySimulation,
    orbit_trails: Vec<VecDeque<Vec3>>,
    orbit_forecasts: Vec<Vec<DVec3>>,
    moon_orbit_offsets: Vec<(Entity, Vec<DVec3>)>,
    orbit_segments: Vec<OrbitSegment>,
    forecast_worker: OrbitForecastWorker,
    start_time: Instant,
    last_physics_update: Instant,
    last_orbit_sample_year: f64,
    last_orbit_forecast_request: Instant,
    orbit_segments_dirty: bool,
    fps_frame_count: u32,
    fps_last_update: Instant,
    current_fps: f64,
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
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
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
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
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
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
        let orbit_trails = create_orbit_trails(&physics);

        let mut object_gpu = Vec::with_capacity(world.entity_capacity());
        for entity in world.entities() {
            let object_uniform = entity_object_uniform(&world, &physics, entity, 0.0);
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
                * std::mem::size_of::<OrbitSegment>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let orbit_forecasts = physics
            .forecast_full_planet_orbits(ORBIT_FORECAST_MAX_POINTS, ORBIT_FORECAST_SAMPLE_YEARS);
        let moon_orbit_offsets = physics.forecast_full_moon_orbit_offsets(
            MOON_ORBIT_FORECAST_MAX_POINTS,
            MOON_ORBIT_FORECAST_SAMPLE_YEARS,
        );
        let mut orbit_segments = Vec::with_capacity(max_orbit_segment_count(
            &world,
            physics.planet_entities().len(),
        ));
        build_orbit_segments(
            &orbit_trails,
            &orbit_forecasts,
            &moon_orbit_offsets,
            &world,
            &physics,
            physics.planet_entities(),
            orbit_width_scale(&camera),
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
        let planet_pipeline = create_sphere_pipeline(
            &device,
            format,
            &pipeline_layout,
            &planet_shader,
            MSAA_SAMPLE_COUNT,
            "Planet Pipeline",
        );
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

        let orbit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Orbit Pipeline"),
            layout: Some(&orbit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &orbit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &orbit_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
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
        let forecast_worker = OrbitForecastWorker::new();
        let now = Instant::now();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            sun_pipeline,
            planet_pipeline,
            orbit_pipeline,
            text_overlay_pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            orbit_vertex_count,
            orbit_buffer,
            fps_overlay,
            orbit_bind_group,
            camera_buffer,
            camera_bind_group,
            object_gpu,
            msaa,
            depth,
            camera,
            world,
            physics,
            orbit_trails,
            orbit_forecasts,
            moon_orbit_offsets,
            orbit_segments,
            forecast_worker,
            start_time: now,
            last_physics_update: now,
            last_orbit_sample_year: 0.0,
            last_orbit_forecast_request: now,
            orbit_segments_dirty: false,
            fps_frame_count: 0,
            fps_last_update: now,
            current_fps: 0.0,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.msaa = create_msaa_target(&self.device, width, height, self.config.format);
        self.depth = create_depth_target(&self.device, width, height);
    }

    fn orbit_camera(&mut self, delta_x: f64, delta_y: f64) {
        self.camera.orbit(delta_x, delta_y);
    }

    fn pan_camera(&mut self, delta_x: f64, delta_y: f64) {
        self.camera.pan(delta_x, delta_y, self.config.height);
    }

    fn zoom_camera(&mut self, scroll_delta: f32) {
        self.camera.zoom(scroll_delta);
        self.orbit_segments_dirty = true;
    }

    fn update_orbit_trails(&mut self) -> bool {
        let elapsed_years = self.physics.elapsed_years();
        if elapsed_years - self.last_orbit_sample_year < ORBIT_TRAIL_SAMPLE_YEARS {
            return false;
        }

        self.last_orbit_sample_year = elapsed_years;
        for (index, trail) in self.orbit_trails.iter_mut().enumerate() {
            if trail.len() == ORBIT_TRAIL_POINTS {
                trail.pop_front();
            }
            trail.push_back(dvec3_to_vec3(self.physics.planet_position(index)));
        }

        true
    }

    fn poll_orbit_forecasts(&mut self) {
        let Some(result) = self.forecast_worker.poll() else {
            return;
        };

        self.orbit_forecasts = result.orbit_forecasts;
        self.moon_orbit_offsets = result.moon_orbit_offsets;
        self.orbit_segments_dirty = true;
    }

    fn request_orbit_forecasts_if_needed(&mut self, now: Instant) {
        if now
            .duration_since(self.last_orbit_forecast_request)
            .as_secs_f64()
            < ORBIT_FORECAST_UPDATE_INTERVAL_SECONDS
        {
            return;
        }

        if self.forecast_worker.request(&self.physics) {
            self.last_orbit_forecast_request = now;
        }
    }

    fn upload_orbit_segments(&mut self) {
        self.orbit_segments.clear();
        build_orbit_segments(
            &self.orbit_trails,
            &self.orbit_forecasts,
            &self.moon_orbit_offsets,
            &self.world,
            &self.physics,
            self.physics.planet_entities(),
            orbit_width_scale(&self.camera),
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
        self.orbit_segments_dirty = false;
    }

    fn toggle_borderless_fullscreen(&self) {
        let fullscreen = if self.window.fullscreen().is_some() {
            None
        } else {
            Some(Fullscreen::Borderless(None))
        };

        self.window.set_fullscreen(fullscreen);
    }

    fn handle_shader_key(&mut self, key: KeyCode) -> bool {
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
        self.update_window_title();
        true
    }

    fn update_window_title(&self) {
        let Some(planet) = self.world.first_entity_of_kind(CelestialKind::Planet) else {
            return;
        };
        let Some(surface) = self.world.surface_material(planet) else {
            return;
        };
        let atmosphere_density = self
            .world
            .atmosphere(planet)
            .map_or(0.0, |atmosphere| atmosphere.density);
        let star = self.world.first_entity_of_kind(CelestialKind::Star);
        let star_material = star.and_then(|entity| self.world.star_material(entity));
        let planet_count = self.world.count_kind(CelestialKind::Planet);
        let moon_count = self.world.count_kind(CelestialKind::Moon);
        self.window.set_title(&format!(
            "Solar WGPU | N-body {:.2} y | planets {} moons {} | Planet Q/A rough {:.2} W/S metal {:.2} E/D atm {:.2} | Sun R/F bright {:.2} T/G temp {:.0}K",
            self.physics.elapsed_years(),
            planet_count,
            moon_count,
            surface.roughness,
            surface.metallic,
            atmosphere_density,
            star_material.map_or(0.0, |material| material.brightness),
            star_material.map_or(0.0, |material| material.surface_temperature)
        ));
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
        self.update_window_title();
    }

    fn render(&mut self) {
        let now = Instant::now();
        let frame_seconds = now.duration_since(self.last_physics_update).as_secs_f64();
        self.last_physics_update = now;
        self.physics.advance(frame_seconds);
        self.update_orbit_trails();

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

        self.poll_orbit_forecasts();
        self.request_orbit_forecasts_if_needed(now);
        self.upload_orbit_segments();

        let elapsed = self.start_time.elapsed().as_secs_f32();
        for object_gpu in &self.object_gpu {
            let uniform =
                entity_object_uniform(&self.world, &self.physics, object_gpu.entity, elapsed);
            self.queue.write_buffer(
                &object_gpu.object_buffer,
                0,
                bytemuck::cast_slice(&[uniform]),
            );
        }

        self.fps_overlay.update(
            &self.queue,
            self.current_fps,
            self.config.width,
            self.config.height,
        );

        let mut encoder = self.device.create_command_encoder(&Default::default());

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

            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            pass.set_pipeline(&self.sun_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            for object_gpu in self
                .object_gpu
                .iter()
                .filter(|object| self.world.kind(object.entity) == CelestialKind::Star)
            {
                pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }

            pass.set_pipeline(&self.orbit_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.orbit_bind_group, &[]);
            pass.draw(0..self.orbit_vertex_count, 0..1);

            pass.set_pipeline(&self.planet_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for object_gpu in self.object_gpu.iter().filter(|object| {
                matches!(
                    self.world.kind(object.entity),
                    CelestialKind::Planet | CelestialKind::Moon
                )
            }) {
                pass.set_bind_group(1, &object_gpu.object_bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }

            if self.fps_overlay.text_vertex_count > 0 {
                pass.set_pipeline(&self.text_overlay_pipeline);
                pass.set_bind_group(0, &self.fps_overlay.text_bind_group, &[]);
                pass.set_vertex_buffer(0, self.fps_overlay.text_vertex_buffer.slice(..));
                pass.draw(0..self.fps_overlay.text_vertex_count, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        self.update_fps_counter(Instant::now());
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
    rotating_world: bool,
    panning_map: bool,
    last_cursor: Option<(f64, f64)>,
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
            state.update_window_title();
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

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
            }

            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if key == KeyCode::F11 {
                        state.toggle_borderless_fullscreen();
                    } else if state.handle_shader_key(key) {
                        state.window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
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
            } => {
                self.rotating_world = button_state == ElementState::Pressed;
                self.last_cursor = None;
            }

            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Left,
                ..
            } => {
                self.panning_map = button_state == ElementState::Pressed;
                self.last_cursor = None;
            }

            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x, position.y);
                if self.panning_map {
                    if let Some((last_x, last_y)) = self.last_cursor {
                        state.pan_camera(current.0 - last_x, current.1 - last_y);
                    }
                    state.window.request_redraw();
                } else if self.rotating_world {
                    if let Some((last_x, last_y)) = self.last_cursor {
                        state.orbit_camera(current.0 - last_x, current.1 - last_y);
                    }
                    state.window.request_redraw();
                }
                self.last_cursor = Some(current);
            }

            WindowEvent::RedrawRequested => {
                state.render();
                state.window.request_redraw();
            }

            _ => {}
        }
    }
}

fn create_sphere_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VERTEX_ATTRIBUTES,
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

fn create_text_overlay_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    text_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Text Overlay Pipeline Layout"),
        bind_group_layouts: &[Some(text_bind_group_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Text Overlay Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<TextOverlayVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &TEXT_OVERLAY_VERTEX_ATTRIBUTES,
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

fn create_msaa_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> MsaaTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MSAA Color Texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());

    MsaaTarget {
        _texture: texture,
        view,
    }
}

fn create_depth_target(device: &wgpu::Device, width: u32, height: u32) -> DepthTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());

    DepthTarget {
        _texture: texture,
        view,
    }
}

fn rasterize_google_sans_text(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text_buffer: &mut TextBuffer,
    font_family: &str,
    text: &str,
) -> Vec<u8> {
    let mut pixels = vec![0; (FPS_TEXT_TEXTURE_WIDTH * FPS_TEXT_TEXTURE_HEIGHT * 4) as usize];
    let attrs = Attrs::new().family(Family::Name(font_family));
    text_buffer.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    text_buffer.shape_until_scroll(font_system, true);
    text_buffer.draw(
        font_system,
        swash_cache,
        TextColor::rgba(210, 245, 255, 255),
        |x, y, width, height, color| {
            let [red, green, blue, alpha] = color.as_rgba();
            for row in 0..height as i32 {
                for column in 0..width as i32 {
                    let pixel_x = x + column;
                    let pixel_y = y + row;
                    if pixel_x < 0
                        || pixel_y < 0
                        || pixel_x >= FPS_TEXT_TEXTURE_WIDTH as i32
                        || pixel_y >= FPS_TEXT_TEXTURE_HEIGHT as i32
                    {
                        continue;
                    }

                    let index =
                        ((pixel_y as u32 * FPS_TEXT_TEXTURE_WIDTH + pixel_x as u32) * 4) as usize;
                    pixels[index] = red;
                    pixels[index + 1] = green;
                    pixels[index + 2] = blue;
                    pixels[index + 3] = alpha;
                }
            }
        },
    );

    pixels
}

fn build_fps_text_vertices(viewport_width: u32, viewport_height: u32) -> Vec<TextOverlayVertex> {
    let x = (viewport_width as f32 - FPS_TEXT_TEXTURE_WIDTH as f32 - FPS_OVERLAY_MARGIN).max(0.0);
    let y = FPS_OVERLAY_MARGIN.min(
        (viewport_height as f32 - FPS_TEXT_TEXTURE_HEIGHT as f32 - FPS_OVERLAY_MARGIN).max(0.0),
    );
    let mut vertices = Vec::with_capacity(TEXT_OVERLAY_MAX_VERTICES);
    push_textured_screen_rect(
        &mut vertices,
        x,
        y,
        FPS_TEXT_TEXTURE_WIDTH as f32,
        FPS_TEXT_TEXTURE_HEIGHT as f32,
        viewport_width,
        viewport_height,
    );
    vertices
}

fn push_textured_screen_rect(
    vertices: &mut Vec<TextOverlayVertex>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    viewport_width: u32,
    viewport_height: u32,
) {
    let left = screen_x_to_clip(x, viewport_width);
    let right = screen_x_to_clip(x + width, viewport_width);
    let top = screen_y_to_clip(y, viewport_height);
    let bottom = screen_y_to_clip(y + height, viewport_height);

    vertices.extend_from_slice(&[
        text_overlay_vertex(left, top, 0.0, 0.0),
        text_overlay_vertex(left, bottom, 0.0, 1.0),
        text_overlay_vertex(right, bottom, 1.0, 1.0),
        text_overlay_vertex(left, top, 0.0, 0.0),
        text_overlay_vertex(right, bottom, 1.0, 1.0),
        text_overlay_vertex(right, top, 1.0, 0.0),
    ]);
}

fn text_overlay_vertex(x: f32, y: f32, u: f32, v: f32) -> TextOverlayVertex {
    [x, y, u, v]
}

fn screen_x_to_clip(x: f32, viewport_width: u32) -> f32 {
    x / viewport_width.max(1) as f32 * 2.0 - 1.0
}

fn screen_y_to_clip(y: f32, viewport_height: u32) -> f32 {
    1.0 - y / viewport_height.max(1) as f32 * 2.0
}

fn object_uniform(
    model: Mat4,
    base_color: [f32; 3],
    accent_color: [f32; 3],
    emissive: f32,
    shader_params: [f32; 4],
) -> ObjectUniform {
    let mut uniform = [0.0; 32];
    uniform[..16].copy_from_slice(&model.to_cols_array());
    uniform[16..20].copy_from_slice(&[base_color[0], base_color[1], base_color[2], 1.0]);
    uniform[20..24].copy_from_slice(&[accent_color[0], accent_color[1], accent_color[2], 1.0]);
    uniform[24..28].copy_from_slice(&[emissive, 0.0, 0.0, 0.0]);
    uniform[28..32].copy_from_slice(&shader_params);
    uniform
}

fn entity_object_uniform(
    world: &World,
    physics: &NBodySimulation,
    entity: Entity,
    shader_time: f32,
) -> ObjectUniform {
    let body = world.body(entity);
    let rotation = world.rotation(entity);
    let render = world.render(entity);
    let position = dvec3_to_vec3(physics.position(entity));
    let model = Mat4::from_translation(position)
        * Mat4::from_rotation_y(shader_time * rotation.speed)
        * Mat4::from_scale(Vec3::splat(body.radius));

    match render.material {
        MaterialComponent::Star(material) => object_uniform(
            model,
            material.base_color.as_array(),
            material.accent_color.as_array(),
            material.brightness,
            [
                ((material.surface_temperature - 2500.0) / 9500.0).clamp(0.0, 1.0),
                1.35,
                18.0,
                shader_time,
            ],
        ),
        MaterialComponent::Surface(material) => {
            let atmosphere = world.atmosphere(entity);
            let accent_color =
                atmosphere.map_or(material.accent_color, |atmosphere| atmosphere.color);
            let atmosphere_density = atmosphere.map_or(0.0, |atmosphere| {
                atmosphere.density * atmosphere.radius_multiplier.max(0.0)
            });
            object_uniform(
                model,
                material.base_color.as_array(),
                accent_color.as_array(),
                0.0,
                [
                    material.roughness,
                    material.metallic,
                    atmosphere_density,
                    shader_time,
                ],
            )
        }
    }
}

fn dvec3_to_vec3(position: DVec3) -> Vec3 {
    Vec3::new(position.x as f32, position.y as f32, position.z as f32)
}
fn create_world() -> World {
    let mut world = World::default();
    let star = world.spawn(star_bundle());
    let specs = [
        (
            "Aurelia",
            0.23,
            1.0,
            2.7,
            2.05,
            0.55,
            0.00,
            Color::rgb(0.10, 0.34, 1.00),
        ),
        (
            "Vesta",
            0.16,
            0.35,
            1.55,
            1.42,
            0.85,
            0.12,
            Color::rgb(0.85, 0.46, 0.18),
        ),
        (
            "Nereid",
            0.19,
            1.8,
            3.65,
            3.15,
            0.38,
            -0.18,
            Color::rgb(0.22, 0.78, 0.74),
        ),
        (
            "Icarus",
            0.12,
            0.18,
            1.05,
            0.92,
            1.20,
            0.04,
            Color::rgb(0.76, 0.24, 0.12),
        ),
        (
            "Boreas",
            0.28,
            28.0,
            4.65,
            4.10,
            0.25,
            0.28,
            Color::rgb(0.45, 0.68, 0.92),
        ),
        (
            "Nyx",
            0.14,
            6.0,
            5.35,
            4.75,
            0.18,
            -0.32,
            Color::rgb(0.42, 0.36, 0.68),
        ),
    ];

    for (index, (name, radius, earth_masses, major, minor, speed, inclination, color)) in
        specs.into_iter().enumerate()
    {
        let mut orbit = PlanetOrbit::elliptical(major, minor, speed);
        orbit.phase = index as f32 * 0.85;
        orbit.inclination = inclination;

        let planet = world.spawn(ObjectBundle {
            name: name.to_string(),
            kind: CelestialKind::Planet,
            parent: Some(star),
            body: BodyComponent::new(EARTH_MASS_KG * earth_masses, radius, Some(orbit)),
            rotation: RotationComponent {
                speed: 0.7 + index as f32 * 0.18,
            },
            render: RenderComponent {
                material: MaterialComponent::Surface(SurfaceMaterial {
                    base_color: color,
                    accent_color: Color::rgb(0.55, 0.85, 1.0),
                    roughness: 0.65 + index as f32 * 0.04,
                    metallic: 0.02,
                }),
            },
            atmosphere: Some(AtmosphereComponent::new(
                Color::rgb(0.45, 0.72, 1.0),
                0.20 + index as f32 * 0.03,
                1.08,
            )),
        });

        for moon in create_moons_for_planet(index, planet) {
            world.spawn(moon);
        }
    }

    world
}

fn star_bundle() -> ObjectBundle {
    ObjectBundle {
        name: "Sol".to_string(),
        kind: CelestialKind::Star,
        parent: None,
        body: BodyComponent::new(1.989e30, 1.0, None),
        rotation: RotationComponent { speed: 0.15 },
        render: RenderComponent {
            material: MaterialComponent::Star(StarMaterial {
                base_color: Color::rgb(1.0, 0.72, 0.08),
                accent_color: Color::rgb(1.0, 0.92, 0.2),
                brightness: 1.0,
                surface_temperature: 5778.0,
            }),
        },
        atmosphere: None,
    }
}

fn create_moons_for_planet(planet_index: usize, parent: Entity) -> Vec<ObjectBundle> {
    match planet_index {
        0 => vec![make_moon(
            parent,
            "Luma",
            0.045,
            0.36,
            32.0,
            0.18,
            0.40,
            Color::rgb(0.62, 0.63, 0.59),
        )],
        1 => vec![make_moon(
            parent,
            "Cinder",
            0.030,
            0.27,
            -44.0,
            -0.10,
            1.70,
            Color::rgb(0.56, 0.42, 0.34),
        )],
        2 => vec![
            make_moon(
                parent,
                "Nami",
                0.038,
                0.35,
                28.0,
                0.22,
                0.20,
                Color::rgb(0.70, 0.76, 0.78),
            ),
            make_moon(
                parent,
                "Thalassa",
                0.030,
                0.52,
                -18.0,
                -0.16,
                2.40,
                Color::rgb(0.45, 0.58, 0.64),
            ),
        ],
        3 => vec![make_moon(
            parent,
            "Pyra",
            0.026,
            0.23,
            55.0,
            0.05,
            2.80,
            Color::rgb(0.67, 0.50, 0.42),
        )],
        4 => vec![
            make_moon(
                parent,
                "Caldus",
                0.060,
                0.46,
                22.0,
                0.25,
                0.90,
                Color::rgb(0.72, 0.66, 0.56),
            ),
            make_moon(
                parent,
                "Rime",
                0.045,
                0.63,
                -15.0,
                -0.18,
                2.20,
                Color::rgb(0.76, 0.82, 0.88),
            ),
            make_moon(
                parent,
                "Aster",
                0.034,
                0.82,
                10.0,
                0.34,
                3.60,
                Color::rgb(0.50, 0.48, 0.44),
            ),
        ],
        5 => vec![
            make_moon(
                parent,
                "Umbra",
                0.035,
                0.30,
                34.0,
                -0.25,
                1.10,
                Color::rgb(0.40, 0.42, 0.50),
            ),
            make_moon(
                parent,
                "Nyxis",
                0.030,
                0.46,
                -21.0,
                0.20,
                2.90,
                Color::rgb(0.60, 0.58, 0.68),
            ),
        ],
        _ => Vec::new(),
    }
}

fn make_moon(
    parent: Entity,
    name: &str,
    radius: f32,
    orbit_radius: f32,
    angular_speed: f32,
    inclination: f32,
    phase: f32,
    color: Color,
) -> ObjectBundle {
    let mut orbit = PlanetOrbit::circular(orbit_radius, angular_speed);
    orbit.phase = phase;
    orbit.inclination = inclination;

    let mass_scale = (radius / 0.045).max(0.25).powi(3);
    ObjectBundle {
        name: name.to_string(),
        kind: CelestialKind::Moon,
        parent: Some(parent),
        body: BodyComponent::new(LUNAR_MASS_KG * mass_scale, radius, Some(orbit)),
        rotation: RotationComponent {
            speed: 0.65 + radius * 6.0,
        },
        render: RenderComponent {
            material: MaterialComponent::Surface(SurfaceMaterial {
                base_color: color,
                accent_color: Color::rgb(0.70, 0.72, 0.76),
                roughness: 0.88,
                metallic: 0.0,
            }),
        },
        atmosphere: None,
    }
}

fn create_sphere(latitudes: u32, longitudes: u32, radius: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(((latitudes + 1) * (longitudes + 1)) as usize);
    let mut indices = Vec::with_capacity((latitudes * longitudes * 6) as usize);
    let radius = radius.max(0.001);

    for lat in 0..=latitudes {
        let theta = lat as f32 / latitudes as f32 * PI;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=longitudes {
            let phi = lon as f32 / longitudes as f32 * TAU;
            vertices.push([
                radius * sin_theta * phi.cos(),
                radius * cos_theta,
                radius * sin_theta * phi.sin(),
            ]);
        }
    }

    let stride = longitudes + 1;
    for lat in 0..latitudes {
        for lon in 0..longitudes {
            let top_left = lat * stride + lon;
            let top_right = top_left + 1;
            let bottom_left = top_left + stride;
            let bottom_right = bottom_left + 1;

            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
        }
    }

    (vertices, indices)
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
