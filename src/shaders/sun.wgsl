struct Camera {
    view_proj: mat4x4<f32>,
};

struct Object {
    model: mat4x4<f32>,
    base_color: vec4<f32>,
    accent_color: vec4<f32>,
    emissive: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<storage, read> objects: array<Object>;

struct VertexIn {
    @location(0) position: vec3<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) local_pos: vec3<f32>,
    @location(3) base_color: vec4<f32>,
    @location(4) accent_color: vec4<f32>,
    @location(5) emissive: vec4<f32>,
    @location(6) params: vec4<f32>,
};

@vertex
fn vs_main(in: VertexIn, @builtin(instance_index) instance_index: u32) -> VertexOut {
    let object = objects[instance_index];
    let world = object.model * vec4<f32>(in.position, 1.0);

    var out: VertexOut;
    out.clip_position = camera.view_proj * world;
    out.normal = normalize((object.model * vec4<f32>(in.position, 0.0)).xyz);
    out.world_pos = world.xyz;
    out.local_pos = in.position;
    out.base_color = object.base_color;
    out.accent_color = object.accent_color;
    out.emissive = object.emissive;
    out.params = object.params;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let temperature_mix = in.params.x;
    let pulse_speed = in.params.y;
    let noise_scale = in.params.z;
    let time = in.params.w;

    let hot_color = vec3<f32>(1.0, 0.96, 0.62);
    let cool_color = in.base_color.rgb;
    let core_color = mix(cool_color, hot_color, temperature_mix);

    let flow_a = sin((in.local_pos.x + in.local_pos.y * 0.72) * noise_scale + time * pulse_speed);
    let flow_b = sin(
        (in.local_pos.z * 1.65 - in.local_pos.y) * noise_scale * 1.31
            - time * pulse_speed * 0.63
            + flow_a * 1.2,
    );
    let granules = sin(
        in.local_pos.x * noise_scale * 2.4
            + sin(in.local_pos.z * noise_scale * 1.8 + time * 0.21) * 1.5,
    );
    let surface_mix = clamp(0.48 + flow_a * 0.18 + flow_b * 0.17 + granules * 0.10, 0.0, 1.0);
    let surface_color = mix(core_color * 0.92, in.accent_color.rgb * 1.12, surface_mix);

    let front_light = 0.82 + max(dot(n, normalize(vec3<f32>(-0.35, 0.55, 0.75))), 0.0) * 0.18;
    let view_facing = max(dot(n, normalize(-in.world_pos)), 0.0);
    let rim = pow(1.0 - view_facing, 2.2);
    let corona = in.accent_color.rgb * rim * (0.44 + temperature_mix * 0.72);
    let center_glow = hot_color * pow(view_facing, 1.5) * 0.12;
    let brightness = max(in.emissive.x, 0.0);
    let selection_brightness = max(in.emissive.z, 0.0);

    return vec4<f32>(
        (surface_color * front_light + corona + center_glow) * brightness * selection_brightness,
        1.0,
    );
}
