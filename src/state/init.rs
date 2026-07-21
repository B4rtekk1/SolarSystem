use super::*;

impl State {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("Failed to create GPU surface: {error}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("No compatible GPU adapter found: {error}"))?;

        let adapter_features = adapter.features();
        let required_features = wgpu::Features::INDIRECT_FIRST_INSTANCE;
        if !adapter_features.contains(required_features) {
            return Err("GPU adapter does not support INDIRECT_FIRST_INSTANCE".to_string());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features,
                ..Default::default()
            })
            .await
            .map_err(|error| format!("Failed to create GPU device: {error}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let sun_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sun Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sun.wgsl").into()),
        });
        let planet_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Planet Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/planet.wgsl").into()),
        });
        let orbit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Orbit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/orbit.wgsl").into()),
        });
        let text_overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text_overlay.wgsl").into()),
        });
        let screen_dim_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Screen Dim Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/screen_dim.wgsl").into()),
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
                entries: &[read_only_storage_buffer_layout_entry(
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

        let object_uniforms: Vec<ObjectUniform> = world
            .entities()
            .map(|entity| entity_object_uniform(&world, &physics, entity, 0.0, None))
            .collect();
        let object_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Object Storage Buffer"),
            contents: bytemuck::cast_slice(&object_uniforms),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Object Storage Bind Group"),
            layout: &object_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: object_buffer.as_entire_binding(),
            }],
        });

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
        let mut indirect_commands: Vec<IndexedIndirectArgs> = world
            .entities()
            .map(|entity| indexed_indirect_args(index_count, 1, entity.index() as u32))
            .collect();
        let star_batches = append_kind_batches(
            &world,
            CelestialKind::Star,
            index_count,
            &mut indirect_commands,
        );
        let planet_batches = append_kind_batches(
            &world,
            CelestialKind::Planet,
            index_count,
            &mut indirect_commands,
        );
        let moon_batches = append_kind_batches(
            &world,
            CelestialKind::Moon,
            index_count,
            &mut indirect_commands,
        );
        let indirect_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Indexed Indirect Buffer"),
            contents: bytemuck::cast_slice(&indirect_commands),
            usage: wgpu::BufferUsages::INDIRECT,
        });
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
        configure_egui(&egui_ctx);
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

        Ok(Self {
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
            orbit_vertex_count,
            orbit_buffer,
            fps_overlay,
            egui_ctx,
            egui_winit,
            egui_renderer,
            orbit_bind_group,
            camera_buffer,
            camera_bind_group,
            object_buffer,
            object_bind_group,
            object_uniforms,
            indirect_buffer,
            star_batches,
            planet_batches,
            moon_batches,
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
            body_search: String::new(),
            metrics_history: VecDeque::new(),
        })
    }
}
