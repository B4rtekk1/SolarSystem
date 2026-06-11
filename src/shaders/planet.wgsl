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
    let light_dir = normalize(-in.world_pos);
    let view_dir = normalize(-in.world_pos);

    let roughness = clamp(object.params.x, 0.02, 1.0);
    let metallic = clamp(object.params.y, 0.0, 1.0);
    let atmosphere_density = clamp(object.params.z, 0.0, 1.5);
    let time = object.params.w;
    let highlight = clamp(object.emissive.y, 0.0, 1.0);

    let latitude_band = 0.5 + 0.5 * sin(in.local_pos.y * 18.0 + in.local_pos.x * 7.0 + time * 0.08);
    let land_noise = 0.5 + 0.5 * sin((in.local_pos.x * 1.9 + in.local_pos.z) * 22.0);
    let coast_noise = 0.5 + 0.5 * sin((in.local_pos.z - in.local_pos.y * 0.6) * 31.0);
    let terrain = clamp(latitude_band * 0.35 + land_noise * 0.45 + coast_noise * 0.2, 0.0, 1.0);

    let water = object.base_color.rgb;
    let land = mix(vec3<f32>(0.12, 0.34, 0.18), object.accent_color.rgb, metallic * 0.45);
    let surface_color = mix(water, land, smoothstep(0.48, 0.62, terrain));

    let diffuse = max(dot(n, light_dir), 0.0);
    let ambient = 0.12 + (1.0 - roughness) * 0.08;
    let half_dir = normalize(light_dir + view_dir);
    let spec_power = mix(16.0, 96.0, 1.0 - roughness);
    let specular = pow(max(dot(n, half_dir), 0.0), spec_power) * mix(0.12, 0.65, metallic);

    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 2.2);
    let atmosphere = object.accent_color.rgb * rim * atmosphere_density * 0.32;
    let lit_surface = surface_color * (ambient + diffuse * 0.88) + specular + atmosphere;
    let highlight_color = mix(object.accent_color.rgb, vec3<f32>(1.0, 0.94, 0.58), 0.45);
    let highlighted_surface = mix(lit_surface, lit_surface + highlight_color * (0.24 + rim * 1.15), highlight);

    return vec4<f32>(highlighted_surface, 1.0);
}
