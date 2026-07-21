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
    let light_dir = normalize(-in.world_pos);
    let view_dir = normalize(-in.world_pos);

    let roughness = clamp(in.params.x, 0.02, 1.0);
    let metallic = clamp(in.params.y, 0.0, 1.0);
    let atmosphere_density = clamp(in.params.z, 0.0, 1.5);
    let time = in.params.w;
    let highlight = clamp(in.emissive.y, 0.0, 1.0);
    let selection_brightness = max(in.emissive.z, 0.0);

    // Layered, softly warped bands create readable surface detail without textures.
    let longitude = atan2(in.local_pos.z, in.local_pos.x);
    let latitude = asin(clamp(in.local_pos.y, -1.0, 1.0));
    let slow_drift = time * 0.035;
    let warp = sin(longitude * 3.0 - slow_drift) * 0.16
        + sin(longitude * 9.0 + latitude * 4.0 + slow_drift * 0.7) * 0.055;
    let broad_bands = 0.5 + 0.5 * sin((latitude + warp) * 11.0);
    let fine_bands = 0.5 + 0.5 * sin((latitude - warp * 0.35) * 31.0 + longitude * 1.7);
    let cellular = 0.5 + 0.5 * sin(
        in.local_pos.x * 21.0
            + sin(in.local_pos.z * 17.0 - slow_drift) * 2.4
            - in.local_pos.y * 13.0,
    );
    let detail = clamp(broad_bands * 0.48 + fine_bands * 0.20 + cellular * 0.32, 0.0, 1.0);
    let surface_mask = smoothstep(0.34, 0.72, detail);

    let shadow_color = in.base_color.rgb * vec3<f32>(0.38, 0.43, 0.54);
    let light_color = mix(in.base_color.rgb * 1.16, in.accent_color.rgb, 0.28);
    let surface_color = mix(shadow_color, light_color, surface_mask);

    let light_amount = dot(n, light_dir);
    let diffuse = smoothstep(-0.10, 0.26, light_amount);
    let ambient = 0.055 + (1.0 - roughness) * 0.055;
    let half_dir = normalize(light_dir + view_dir);
    let spec_power = mix(16.0, 96.0, 1.0 - roughness);
    let specular = pow(max(dot(n, half_dir), 0.0), spec_power)
        * mix(0.08, 0.56, 1.0 - roughness + metallic * 0.4)
        * diffuse;

    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 2.5);
    let atmosphere = in.accent_color.rgb * rim * atmosphere_density * (0.20 + diffuse * 0.32);
    let night_glow = in.accent_color.rgb * max(-light_amount, 0.0) * 0.012;
    let lit_surface = surface_color * (ambient + diffuse * 0.94) + specular + atmosphere + night_glow;
    let highlight_color = mix(in.accent_color.rgb, vec3<f32>(1.0, 0.94, 0.58), 0.45);
    let highlighted_surface = lit_surface + highlight_color * highlight * (0.08 + rim * 0.28);

    return vec4<f32>(highlighted_surface * selection_brightness, 1.0);
}
