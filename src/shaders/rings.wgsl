struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec4<f32>,
};

struct RingUniform {
    model: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,
};

struct RingParticle {
    offset_size: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> ring: RingUniform;

@group(1) @binding(1)
var<storage, read> particles: array<RingParticle>;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let particle = particles[vertex_index / 6u];
    let corner = vertex_index % 6u;
    let x = select(-1.0, 1.0, corner == 1u || corner == 4u || corner == 5u);
    let y = select(-1.0, 1.0, corner == 2u || corner == 3u || corner == 5u);
    let local = vec2<f32>(x, y);

    let world = ring.model * vec4<f32>(particle.offset_size.xyz, 1.0);
    let clip = camera.view_proj * world;
    let viewport = max(camera.viewport.xy, vec2<f32>(1.0, 1.0));
    let size = particle.offset_size.w * ring.params.w;
    let offset = local * size * 2.0 / viewport * clip.w;

    var out: VertexOut;
    out.clip_position = vec4<f32>(clip.xy + offset, clip.z, clip.w);
    out.color = particle.color * ring.color;
    out.local = local;

    if (clip.w <= 0.0) {
        out.clip_position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        out.color = vec4<f32>(0.0);
    }

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let thickness = clamp(ring.params.x, 0.05, 1.0);
    let dist = length(in.local);
    let glow = 1.0 - smoothstep(thickness * 0.5, thickness, dist);

    let alpha = in.color.a * glow;
    let brightness = 1.35;
    return vec4<f32>(in.color.rgb * brightness, alpha);
}
