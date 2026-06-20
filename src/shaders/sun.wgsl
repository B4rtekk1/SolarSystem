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
var<uniform> object: Object;

struct VertexIn {
    @location(0) position: vec3<f32>,
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) local_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let world = object.model * vec4<f32>(in.position, 1.0);

    var out: VertexOut;
    out.clip_position = camera.view_proj * world;
    out.normal = normalize((object.model * vec4<f32>(in.position, 0.0)).xyz);
    out.world_pos = world.xyz;
    out.local_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let temperature_mix = object.params.x;
    let pulse_speed = object.params.y;
    let noise_scale = object.params.z;
    let time = object.params.w;

    let hot_color = vec3<f32>(1.0, 0.96, 0.62);
    let cool_color = object.base_color.rgb;
    let core_color = mix(cool_color, hot_color, temperature_mix);

    let bands = 0.5 + 0.5 * sin((in.local_pos.x + in.local_pos.y * 0.7) * noise_scale + time * pulse_speed);
    let cells = 0.5 + 0.5 * sin((in.local_pos.z * 1.7 - in.local_pos.y) * noise_scale * 1.35 - time * pulse_speed * 0.7);
    let surface_mix = clamp(bands * 0.45 + cells * 0.35, 0.0, 1.0);
    let surface_color = mix(core_color, object.accent_color.rgb, surface_mix);

    let front_light = 0.74 + max(dot(n, normalize(vec3<f32>(-0.35, 0.55, 0.75))), 0.0) * 0.26;
    let rim = pow(1.0 - max(dot(n, normalize(-in.world_pos)), 0.0), 2.0);
    let flare = object.accent_color.rgb * rim * (0.35 + temperature_mix * 0.65);
    let brightness = max(object.emissive.x, 0.0);
    let selection_brightness = max(object.emissive.z, 0.0);

    return vec4<f32>((surface_color * front_light + flare) * brightness * selection_brightness, 1.0);
}
