use crate::render_utils::{
    alpha_blending_fragment_state, alpha_blending_fragment_targets, depth_stencil_state,
};
use wgpu::util::DeviceExt;

const FIELD_STAR_COUNT: usize = 3800;
const GALAXY_STAR_COUNT: usize = 2400;
const NEBULA_CLOUD_COUNT: usize = 280;
const STAR_RADIUS: f32 = 78.0;
const MIN_STAR_SIZE_PIXELS: f32 = 0.8;
const MAX_STAR_SIZE_PIXELS: f32 = 2.8;
const GALAXY_DISTANCE: f32 = 82.0;
const GALAXY_WIDTH: f32 = 25.0;
const GALAXY_HEIGHT: f32 = 7.4;

type Star = [f32; 8];

pub struct Starfield {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    _buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl Starfield {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Starfield Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/stars.wgsl").into()),
        });
        let stars = create_stars();
        let star_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Starfield Buffer"),
            contents: bytemuck::cast_slice(&stars),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let star_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Starfield Bind Group Layout"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Starfield Bind Group"),
            layout: &star_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: star_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Starfield Pipeline Layout"),
            bind_group_layouts: &[
                Some(camera_bind_group_layout),
                Some(&star_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let fragment_targets = alpha_blending_fragment_targets(format);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Starfield Pipeline"),
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
            depth_stencil: Some(depth_stencil_state(false, wgpu::CompareFunction::Always)),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            _buffer: star_buffer,
            vertex_count: (stars.len() * 6) as u32,
        }
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);
        pass.set_bind_group(1, &self.bind_group, &[]);
        pass.draw(0..self.vertex_count, 0..1);
    }
}

fn create_stars() -> Vec<Star> {
    let mut stars = Vec::with_capacity(FIELD_STAR_COUNT + GALAXY_STAR_COUNT + NEBULA_CLOUD_COUNT);

    stars.extend((0..FIELD_STAR_COUNT).map(|index| {
        let u = random01(index as u32, 11);
        let v = random01(index as u32, 29);
        let brightness = random01(index as u32, 47);
        let size = random01(index as u32, 83);
        let hue = random01(index as u32, 131);

        let z = 1.0 - 2.0 * u;
        let angle = std::f32::consts::TAU * v;
        let radius = (1.0 - z * z).sqrt();
        let position = [
            radius * angle.cos() * STAR_RADIUS,
            z * STAR_RADIUS,
            radius * angle.sin() * STAR_RADIUS,
            mix(MIN_STAR_SIZE_PIXELS, MAX_STAR_SIZE_PIXELS, size),
        ];
        let warmth = hue - 0.5;
        let color = [
            (0.78 + warmth * 0.22).clamp(0.62, 1.0),
            (0.86 + brightness * 0.14).clamp(0.70, 1.0),
            (1.00 - warmth * 0.16).clamp(0.74, 1.0),
            0.38 + brightness * 0.62,
        ];

        [
            position[0],
            position[1],
            position[2],
            position[3],
            color[0],
            color[1],
            color[2],
            color[3],
        ]
    }));

    stars.extend((0..GALAXY_STAR_COUNT).map(create_galaxy_star));
    stars.extend((0..NEBULA_CLOUD_COUNT).map(create_nebula_cloud));
    stars
}

fn create_galaxy_star(index: usize) -> Star {
    let index = index as u32;
    let arm = (index % 4) as f32;
    let spread = random01(index, 211);
    let twist = random01(index, 223);
    let cross = random01(index, 239) - 0.5;
    let haze = random01(index, 251) - 0.5;
    let brightness = random01(index, 263);
    let tint = random01(index, 269);

    let core_bias = spread.powf(1.55);
    let disk_radius = GALAXY_WIDTH * core_bias;
    let angle = arm * std::f32::consts::TAU / 4.0 + core_bias * 7.1 + twist * 0.72;
    let disk_x = angle.cos() * disk_radius + cross * (1.0 + core_bias * 2.1);
    let disk_y = angle.sin() * disk_radius * 0.27 + haze * GALAXY_HEIGHT * (1.0 - core_bias * 0.46);

    let center = normalize3([-0.62, 0.28, -0.73]);
    let right = normalize3([center[2], 0.0, -center[0]]);
    let up = normalize3(cross3(right, center));
    let position = add3(
        scale3(center, GALAXY_DISTANCE),
        add3(scale3(right, disk_x), scale3(up, disk_y)),
    );

    let core_glow = 1.0 - core_bias;
    let size = mix(
        1.0,
        3.6,
        (core_glow * 0.65 + brightness * 0.35).clamp(0.0, 1.0),
    );
    let alpha = (0.18 + brightness * 0.42 + core_glow * 0.50).clamp(0.0, 0.95);
    let magenta = smoothstep(0.20, 0.72, tint);
    let cyan = 1.0 - smoothstep(0.36, 0.92, tint);
    let color = [
        0.46 + core_glow * 0.46 + magenta * 0.20,
        0.58 + brightness * 0.24 + cyan * 0.16,
        0.92 + core_glow * 0.08,
        alpha,
    ];

    [
        position[0],
        position[1],
        position[2],
        size,
        color[0],
        color[1],
        color[2],
        color[3],
    ]
}

fn create_nebula_cloud(index: usize) -> Star {
    let index = index as u32;
    let along = random01(index, 307) * 2.0 - 1.0;
    let lane = random01(index, 311) * 2.0 - 1.0;
    let puff = random01(index, 313);
    let depth = random01(index, 317) * 2.0 - 1.0;
    let tint = random01(index, 331);

    let center = normalize3([-0.62, 0.28, -0.73]);
    let right = normalize3([center[2], 0.0, -center[0]]);
    let up = normalize3(cross3(right, center));
    let curve = along * along * along * 4.2;
    let disk_x = along * GALAXY_WIDTH * 0.95 + curve + depth * 1.8;
    let disk_y = lane * GALAXY_HEIGHT * (0.42 + puff * 0.38) + along.sin() * 1.3;
    let position = add3(
        scale3(center, GALAXY_DISTANCE - 1.5 + depth * 2.0),
        add3(scale3(right, disk_x), scale3(up, disk_y)),
    );

    let center_glow = 1.0 - along.abs().min(1.0);
    let alpha = 0.035 + center_glow * 0.07 + puff * 0.035;
    let color = [
        0.25 + tint * 0.35 + center_glow * 0.25,
        0.34 + (1.0 - tint) * 0.24,
        0.82 + puff * 0.15,
        alpha.min(0.16),
    ];

    [
        position[0],
        position[1],
        position[2],
        mix(13.0, 38.0, puff),
        color[0],
        color[1],
        color[2],
        color[3],
    ]
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

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale3(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length <= f32::EPSILON {
        return [0.0, 0.0, 0.0];
    }

    [v[0] / length, v[1] / length, v[2] / length]
}
