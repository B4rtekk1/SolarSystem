struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec4<f32>,
};

struct Star {
    position_size: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<storage, read> stars: array<Star>;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let star = stars[vertex_index / 6u];
    let corner = vertex_index % 6u;
    let x = select(-1.0, 1.0, corner == 1u || corner == 4u || corner == 5u);
    let y = select(-1.0, 1.0, corner == 2u || corner == 3u || corner == 5u);
    let local = vec2<f32>(x, y);
    let clip = camera.view_proj * vec4<f32>(star.position_size.xyz, 1.0);
    let viewport = max(camera.viewport.xy, vec2<f32>(1.0, 1.0));
    let offset = local * star.position_size.w * 2.0 / viewport * clip.w;

    var out: VertexOut;
    out.clip_position = vec4<f32>(clip.xy + offset, clip.z, clip.w);
    out.color = star.color;
    out.local = local;

    if (clip.w <= 0.0) {
        out.clip_position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        out.color = vec4<f32>(0.0);
    }

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let glow = 1.0 - smoothstep(0.0, 1.0, length(in.local));
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a * glow);
}
