mod camera;
mod color;
mod moon;
mod orbit;
pub mod planet;
pub mod sun;

use std::{
    f32::consts::{PI, TAU},
    sync::Arc,
    time::Instant,
};

use camera::Camera;
use color::Color as PlanetColor;
use glam::{Mat4, Vec3};
use orbit::Orbit as PlanetOrbit;
use planet::{Atmosphere, Planet, PlanetShader};
use sun::Sun;
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
const ORBIT_SEGMENTS: u32 = 192;
const ORBIT_SPEED: f32 = 0.01;
const ZOOM_SPEED: f32 = 0.12;
const MIN_CAMERA_DISTANCE: f32 = 1.35;
const MAX_CAMERA_DISTANCE: f32 = 40.0;
const MSAA_SAMPLE_COUNT: u32 = 4;
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];

type Vertex = [f32; 3];
type CameraUniform = [f32; 16];
type ObjectUniform = [f32; 32];
type OrbitUniform = [f32; 12];

struct DepthTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct MsaaTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct PlanetGpu {
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
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    orbit_vertex_count: u32,
    orbit_buffer: wgpu::Buffer,
    orbit_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    sun_object_buffer: wgpu::Buffer,
    sun_object_bind_group: wgpu::BindGroup,
    planet_gpu: Vec<PlanetGpu>,
    msaa: MsaaTarget,
    depth: DepthTarget,
    camera: Camera,
    sun: Sun,
    planets: Vec<Planet>,
    start_time: Instant,
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
            present_mode: wgpu::PresentMode::Fifo,
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

        let sun = Sun::default();
        let planets = create_planets();

        let sun_object_uniform = object_uniform(
            Mat4::from_scale(Vec3::splat(sun.radius)),
            sun.color.as_array(),
            [1.0, 0.92, 0.2],
            sun.brightness,
            [0.45, 1.35, 18.0, 0.0],
        );
        let sun_object_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Object Buffer"),
            contents: bytemuck::cast_slice(&[sun_object_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sun_object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sun Object Bind Group"),
            layout: &object_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sun_object_buffer.as_entire_binding(),
            }],
        });

        let mut planet_gpu = Vec::with_capacity(planets.len());
        for (index, planet) in planets.iter().enumerate() {
            let object_uniform = planet_object_uniform(planet, 0.0, 0.0);
            let object_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Planet {index} Object Buffer")),
                contents: bytemuck::cast_slice(&[object_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Planet {index} Object Bind Group")),
                layout: &object_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: object_buffer.as_entire_binding(),
                }],
            });
            planet_gpu.push(PlanetGpu {
                object_buffer,
                object_bind_group,
            });
        }

        let orbit_data: Vec<OrbitUniform> = planets
            .iter()
            .map(|planet| orbit_uniform(&planet.orbit, planet.shader.base_color.as_array()))
            .collect();
        let orbit_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Orbit Storage Buffer"),
            contents: bytemuck::cast_slice(&orbit_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
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
                topology: wgpu::PrimitiveTopology::LineList,
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
        let orbit_vertex_count = ORBIT_SEGMENTS * 2;
        let msaa = create_msaa_target(&device, config.width, config.height, format);
        let depth = create_depth_target(&device, config.width, config.height);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            sun_pipeline,
            planet_pipeline,
            orbit_pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            orbit_vertex_count,
            orbit_buffer,
            orbit_bind_group,
            camera_buffer,
            camera_bind_group,
            sun_object_buffer,
            sun_object_bind_group,
            planet_gpu,
            msaa,
            depth,
            camera,
            sun,
            planets,
            start_time: Instant::now(),
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

    fn zoom_camera(&mut self, scroll_delta: f32) {
        self.camera.zoom(scroll_delta);
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
        let Some(planet) = self.planets.first_mut() else {
            return false;
        };

        match key {
            KeyCode::KeyQ => planet.shader.roughness += 0.05,
            KeyCode::KeyA => planet.shader.roughness -= 0.05,
            KeyCode::KeyW => planet.shader.metallic += 0.05,
            KeyCode::KeyS => planet.shader.metallic -= 0.05,
            KeyCode::KeyE => {
                if let Some(atmosphere) = &mut planet.atmosphere {
                    atmosphere.density += 0.05;
                }
            }
            KeyCode::KeyD => {
                if let Some(atmosphere) = &mut planet.atmosphere {
                    atmosphere.density -= 0.05;
                }
            }
            KeyCode::KeyR => self.sun.brightness += 0.1,
            KeyCode::KeyF => self.sun.brightness -= 0.1,
            KeyCode::KeyT => self.sun.surface_temperature += 250.0,
            KeyCode::KeyG => self.sun.surface_temperature -= 250.0,
            _ => return false,
        }

        planet.shader.roughness = planet.shader.roughness.clamp(0.0, 1.0);
        planet.shader.metallic = planet.shader.metallic.clamp(0.0, 1.0);
        if let Some(atmosphere) = &mut planet.atmosphere {
            atmosphere.density = atmosphere.density.clamp(0.0, 1.5);
        }
        self.sun.brightness = self.sun.brightness.clamp(0.1, 4.0);
        self.sun.surface_temperature = self.sun.surface_temperature.clamp(2500.0, 12000.0);
        self.update_window_title();
        true
    }

    fn update_window_title(&self) {
        let Some(planet) = self.planets.first() else {
            return;
        };
        let atmosphere_density = planet
            .atmosphere
            .map_or(0.0, |atmosphere| atmosphere.density);
        self.window.set_title(&format!(
            "Solar WGPU | planets {} | Planet Q/A rough {:.2} W/S metal {:.2} E/D atm {:.2} | Sun R/F bright {:.2} T/G temp {:.0}K",
            self.planets.len(),
            planet.shader.roughness,
            planet.shader.metallic,
            atmosphere_density,
            self.sun.brightness,
            self.sun.surface_temperature
        ));
    }

    fn render(&mut self) {
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

        let orbit_data: Vec<OrbitUniform> = self
            .planets
            .iter()
            .map(|planet| orbit_uniform(&planet.orbit, planet.shader.base_color.as_array()))
            .collect();
        self.queue
            .write_buffer(&self.orbit_buffer, 0, bytemuck::cast_slice(&orbit_data));

        let elapsed = self.start_time.elapsed().as_secs_f32();
        let sun_model = Mat4::from_rotation_y(elapsed * self.sun.rotation_speed)
            * Mat4::from_scale(Vec3::splat(self.sun.radius));
        let sun_uniform = object_uniform(
            sun_model,
            self.sun.color.as_array(),
            [1.0, 0.92, 0.2],
            self.sun.brightness,
            [
                ((self.sun.surface_temperature - 2500.0) / 9500.0).clamp(0.0, 1.0),
                1.35,
                18.0,
                elapsed,
            ],
        );
        self.queue.write_buffer(
            &self.sun_object_buffer,
            0,
            bytemuck::cast_slice(&[sun_uniform]),
        );

        let mut planet_uniforms = Vec::with_capacity(self.planets.len());
        for planet in &self.planets {
            planet_uniforms.push(planet_object_uniform(planet, elapsed, elapsed));
        }
        for (planet_gpu, uniform) in self.planet_gpu.iter().zip(planet_uniforms.iter()) {
            self.queue.write_buffer(
                &planet_gpu.object_buffer,
                0,
                bytemuck::cast_slice(&[*uniform]),
            );
        }

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

            pass.set_pipeline(&self.sun_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.sun_object_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.index_count, 0, 0..1);

            pass.set_pipeline(&self.orbit_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.orbit_bind_group, &[]);
            pass.draw(0..self.orbit_vertex_count, 0..self.planets.len() as u32);

            pass.set_pipeline(&self.planet_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for planet_gpu in &self.planet_gpu {
                pass.set_bind_group(1, &planet_gpu.object_bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
    rotating_world: bool,
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

            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x, position.y);
                if self.rotating_world {
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

fn planet_object_uniform(planet: &Planet, orbit_time: f32, shader_time: f32) -> ObjectUniform {
    let position = planet.orbit.position_at(orbit_time);
    let model = Mat4::from_translation(Vec3::new(position[0], position[1], position[2]))
        * Mat4::from_rotation_y(shader_time * planet.rotation_speed)
        * Mat4::from_scale(Vec3::splat(planet.radius));

    object_uniform(
        model,
        planet.shader.base_color.as_array(),
        [0.55, 0.85, 1.0],
        0.0,
        [
            planet.shader.roughness,
            planet.shader.metallic,
            planet
                .atmosphere
                .map_or(0.0, |atmosphere| atmosphere.density),
            shader_time,
        ],
    )
}

fn orbit_uniform(orbit: &PlanetOrbit, color: [f32; 3]) -> OrbitUniform {
    [
        orbit.center[0],
        orbit.center[1],
        orbit.center[2],
        ORBIT_SEGMENTS as f32,
        orbit.semi_major_axis,
        orbit.semi_minor_axis,
        orbit.phase,
        orbit.inclination,
        color[0],
        color[1],
        color[2],
        0.42,
    ]
}

fn create_planets() -> Vec<Planet> {
    let specs = [
        (
            "Aurelia",
            0.23,
            2.7,
            2.05,
            0.55,
            0.00,
            PlanetColor::rgb(0.10, 0.34, 1.00),
        ),
        (
            "Vesta",
            0.16,
            1.55,
            1.42,
            0.85,
            0.12,
            PlanetColor::rgb(0.85, 0.46, 0.18),
        ),
        (
            "Nereid",
            0.19,
            3.65,
            3.15,
            0.38,
            -0.18,
            PlanetColor::rgb(0.22, 0.78, 0.74),
        ),
        (
            "Icarus",
            0.12,
            1.05,
            0.92,
            1.20,
            0.04,
            PlanetColor::rgb(0.76, 0.24, 0.12),
        ),
        (
            "Boreas",
            0.28,
            4.65,
            4.10,
            0.25,
            0.28,
            PlanetColor::rgb(0.45, 0.68, 0.92),
        ),
        (
            "Nyx",
            0.14,
            5.35,
            4.75,
            0.18,
            -0.32,
            PlanetColor::rgb(0.42, 0.36, 0.68),
        ),
    ];

    specs
        .into_iter()
        .enumerate()
        .map(
            |(index, (name, radius, major, minor, speed, inclination, color))| {
                let mut orbit = PlanetOrbit::elliptical(major, minor, speed);
                orbit.phase = index as f32 * 0.85;
                orbit.inclination = inclination;

                let mut planet = Planet::new(name, 5.972e24, radius, orbit)
                    .with_shader(PlanetShader {
                        shader_path: "planet.wgsl".to_string(),
                        base_color: color,
                        roughness: 0.65 + index as f32 * 0.04,
                        metallic: 0.02,
                    })
                    .with_atmosphere(Atmosphere::new(
                        PlanetColor::rgb(0.45, 0.72, 1.0),
                        0.20 + index as f32 * 0.03,
                        1.08,
                    ));
                planet.rotation_speed = 0.7 + index as f32 * 0.18;
                planet
            },
        )
        .collect()
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
