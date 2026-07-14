use crate::ecs::{Entity, RingComponent, World};
use crate::nbody::NBodySimulation;
use crate::render_utils::{
    alpha_blending_fragment_state, alpha_blending_fragment_targets, depth_stencil_state,
    uniform_buffer_layout_entry,
};
use crate::uniforms::{rendered_entity_position, selection_brightness};
use glam::Mat4;
use std::f32::consts::TAU;
use wgpu::util::DeviceExt;

const DEFAULT_PARTICLE_COUNT: u32 = 2800;
const MIN_PARTICLE_SIZE: f32 = 0.55;
const MAX_PARTICLE_SIZE: f32 = 1.45;
const RING_THICKNESS: f32 = 0.012;

type RingParticle = [f32; 8];
pub type RingUniform = [f32; 24];

struct PlanetRingGpu {
    entity: Entity,
    particle_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    particle_count: u32,
}

pub struct PlanetRingSystem {
    pipeline: wgpu::RenderPipeline,
    rings: Vec<PlanetRingGpu>,
}

impl PlanetRingSystem {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        world: &World,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Planet Ring Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rings.wgsl").into()),
        });

        let ring_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Planet Ring Bind Group Layout"),
                entries: &[
                    uniform_buffer_layout_entry(wgpu::ShaderStages::VERTEX_FRAGMENT),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Planet Ring Pipeline Layout"),
            bind_group_layouts: &[
                Some(camera_bind_group_layout),
                Some(&ring_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let fragment_targets = alpha_blending_fragment_targets(format);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Planet Ring Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(alpha_blending_fragment_state(&shader, &fragment_targets)),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil_state(false, wgpu::CompareFunction::LessEqual)),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let rings = world
            .entities()
            .filter_map(|entity| {
                let ring = world.ring(entity)?;
                let planet_radius = world.body(entity).render_radius;
                let particles = create_ring_particles(ring, planet_radius, entity.index() as u32);
                let particle_count = particles.len() as u32;
                let particle_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{} Ring Particles", world.name(entity))),
                        contents: bytemuck::cast_slice(&particles),
                        usage: wgpu::BufferUsages::STORAGE,
                    });
                let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{} Ring Uniform", world.name(entity))),
                    size: size_of::<RingUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("{} Ring Bind Group", world.name(entity))),
                    layout: &ring_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: particle_buffer.as_entire_binding(),
                        },
                    ],
                });

                Some(PlanetRingGpu {
                    entity,
                    particle_buffer,
                    uniform_buffer,
                    bind_group,
                    particle_count,
                })
            })
            .collect();

        Self { pipeline, rings }
    }

    pub fn update(
        &self,
        queue: &wgpu::Queue,
        world: &World,
        physics: &NBodySimulation,
        rotation_time: f32,
        selected_body: Option<Entity>,
    ) {
        for ring_gpu in &self.rings {
            let Some(ring) = world.ring(ring_gpu.entity) else {
                continue;
            };

            let uniform = ring_uniform(
                world,
                physics,
                ring_gpu.entity,
                ring,
                rotation_time,
                selected_body,
            );
            queue.write_buffer(
                &ring_gpu.uniform_buffer,
                0,
                bytemuck::cast_slice(&[uniform]),
            );
        }
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);

        for ring_gpu in &self.rings {
            pass.set_bind_group(1, &ring_gpu.bind_group, &[]);
            pass.draw(0..ring_gpu.particle_count * 6, 0..1);
        }
    }
}

fn ring_uniform(
    world: &World,
    physics: &NBodySimulation,
    entity: Entity,
    ring: RingComponent,
    rotation_time: f32,
    selected_body: Option<Entity>,
) -> RingUniform {
    let body = world.body(entity);
    let position = rendered_entity_position(world, physics, entity);
    let spin = rotation_time * ring.rotation_speed;
    let model = Mat4::from_translation(position)
        * Mat4::from_rotation_y(spin)
        * Mat4::from_rotation_x(ring.tilt);

    let brightness = selection_brightness(world, entity, selected_body);
    let mut uniform = [0.0; 24];
    uniform[..16].copy_from_slice(&model.to_cols_array());
    uniform[16] = ring.color.r;
    uniform[17] = ring.color.g;
    uniform[18] = ring.color.b;
    uniform[19] = 1.0;
    uniform[20] = body.render_radius * ring.inner_radius_multiplier;
    uniform[21] = body.render_radius * ring.outer_radius_multiplier;
    uniform[22] = spin;
    uniform[23] = brightness;
    uniform
}

fn create_ring_particles(ring: RingComponent, planet_radius: f32, seed: u32) -> Vec<RingParticle> {
    let inner = planet_radius * ring.inner_radius_multiplier;
    let outer = planet_radius * ring.outer_radius_multiplier;
    let count = ring.particle_count.max(1).min(12_000);
    let base_color = ring.color.as_array();

    (0..count)
        .map(|index| {
            let u = random01(index, seed.wrapping_add(17));
            let v = random01(index, seed.wrapping_add(41));
            let w = random01(index, seed.wrapping_add(73));
            let size_mix = random01(index, seed.wrapping_add(109));

            let angle = TAU * u;
            let radius = (inner * inner + (outer * outer - inner * inner) * v).sqrt();
            let height = (w - 0.5) * RING_THICKNESS;
            let radial_t = if outer > inner {
                (radius - inner) / (outer - inner)
            } else {
                0.5
            };

            let band = (radial_t * 28.0 + angle * 2.4).sin() * 0.5 + 0.5;
            let gap = (radial_t * 52.0 - 0.35).sin() * 0.5 + 0.5;
            let alpha = (0.10 + band * 0.28 + gap * 0.18).clamp(0.05, 0.72);
            let tint = 0.82 + radial_t * 0.18;

            [
                radius * angle.cos(),
                height,
                radius * angle.sin(),
                mix(MIN_PARTICLE_SIZE, MAX_PARTICLE_SIZE, size_mix),
                base_color[0] * tint,
                base_color[1] * tint,
                base_color[2] * tint,
                alpha,
            ]
        })
        .collect()
}

fn random01(index: u32, salt: u32) -> f32 {
    let mut value = index.wrapping_mul(747_796_405).wrapping_add(salt);
    value ^= value >> 16;
    value = value.wrapping_mul(2_246_822_519);
    value ^= value >> 13;
    value = value.wrapping_mul(3_266_489_917);
    value ^= value >> 16;
    value as f32 / u32::MAX as f32
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
