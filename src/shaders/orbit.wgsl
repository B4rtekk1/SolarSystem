struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec4<f32>,
};

struct OrbitSegment {
    start: vec4<f32>,
    end: vec4<f32>,
    color: vec4<f32>,
    style: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<storage, read> orbit_segments: array<OrbitSegment>;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) edge: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let segment = orbit_segments[vertex_index / 6u];
    let corner = vertex_index % 6u;
    let start_clip = camera.view_proj * segment.start;
    let end_clip = camera.view_proj * segment.end;
    let viewport = max(camera.viewport.xy, vec2<f32>(1.0, 1.0));
    let start_screen = start_clip.xy / start_clip.w * viewport * 0.5;
    let end_screen = end_clip.xy / end_clip.w * viewport * 0.5;
    let screen_direction = end_screen - start_screen;
    let screen_normal = vec2<f32>(-screen_direction.y, screen_direction.x)
        / max(length(screen_direction), 0.0001);
    let use_end = corner == 1u || corner == 4u || corner == 5u;
    let positive_side = corner == 2u || corner == 3u || corner == 5u;
    let side = select(-1.0, 1.0, positive_side);
    let offset_ndc = screen_normal * side * segment.style.x * 2.0 / viewport;

    var clip_position = start_clip;
    if (use_end) {
        clip_position = end_clip;
    }

    var out: VertexOut;
    let clip_offset = offset_ndc * clip_position.w;
    out.clip_position = vec4<f32>(
        clip_position.x + clip_offset.x,
        clip_position.y + clip_offset.y,
        clip_position.z,
        clip_position.w,
    );
    out.color = segment.color;
    out.edge = side;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let feather = 1.0 - smoothstep(0.58, 1.0, abs(in.edge));
    let glow = 0.82 + feather * 0.28;
    return vec4<f32>(in.color.rgb * glow, in.color.a * feather);
}
