struct Camera {
    view_proj: mat4x4<f32>,
};

struct Orbit {
    center: vec4<f32>,
    axes_phase_inclination: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<storage, read> orbits: array<Orbit>;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    let orbit = orbits[instance_index];
    let segment = vertex_index / 2u;
    let endpoint = vertex_index % 2u;
    let segment_count = orbit.center.w;
    let angle = (f32(segment + endpoint) / segment_count) * 6.28318530718 + orbit.axes_phase_inclination.z;

    let semi_major = orbit.axes_phase_inclination.x;
    let semi_minor = orbit.axes_phase_inclination.y;
    let inclination = orbit.axes_phase_inclination.w;
    let raw_x = semi_major * cos(angle);
    let raw_z = semi_minor * sin(angle);
    let sin_i = sin(inclination);
    let cos_i = cos(inclination);

    let position = vec3<f32>(
        orbit.center.x + raw_x,
        orbit.center.y - raw_z * sin_i,
        orbit.center.z + raw_z * cos_i,
    );

    var out: VertexOut;
    out.clip_position = camera.view_proj * vec4<f32>(position, 1.0);
    out.color = orbit.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
