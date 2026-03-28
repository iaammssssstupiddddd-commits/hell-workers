#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings as view_bindings

struct CharacterMaterial {
    base_color: vec4<f32>,
    secondary_color: vec4<f32>,
    sun_light_dir: vec4<f32>,
    uv_scale: vec2<f32>,
    uv_offset: vec2<f32>,
    alpha_cutoff: f32,
    ghost_alpha: f32,
    rim_strength: f32,
    posterize_steps: f32,
    curve_strength: f32,
    material_kind: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: CharacterMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var color_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_UVS_A
    let uv = in.uv * material.uv_scale + material.uv_offset;
#else
    let uv = material.uv_offset;
#endif
    let tex = textureSample(color_texture, color_sampler, uv);

    if material.material_kind < 0.5 {
        let color = tex * material.base_color;
        if color.a <= material.alpha_cutoff {
            discard;
        }
        return color;
    }

    let n = normalize(in.world_normal);
    let v = normalize(view_bindings::view.world_position - in.world_position.xyz);
    let uv_center = (uv - vec2<f32>(0.5, 0.56)) * vec2<f32>(1.55, 1.1);
    let radial = clamp(1.0 - length(uv_center) * 1.25, 0.0, 1.0);
    let round_mask = smoothstep(0.0, 0.82, radial);
    let sun_dir = normalize(material.sun_light_dir.xyz);
    let pseudo_n = normalize(vec3<f32>(
        uv_center.x * 1.35,
        0.85 - abs(uv_center.y) * 0.45,
        0.75 + round_mask * 1.4
    ));
    let curved_up = normalize(vec3<f32>(n.x * 0.15, 1.0, n.z * 0.15));
    let curved_sun = normalize(mix(sun_dir, pseudo_n, 0.72) + vec3<f32>(0.0, 0.35, 0.0));
    let curved_n = normalize(mix(
        n,
        normalize(mix(curved_up, curved_sun, 0.88)),
        material.curve_strength
    ));
    let top_light = clamp(dot(curved_n, sun_dir) * 0.5 + 0.5, 0.0, 1.0);
    let steps = max(material.posterize_steps, 1.0);
    let stepped = floor(top_light * steps + 0.48) / max(steps - 1.0, 1.0);
    let soft_band = smoothstep(0.12, 0.9, top_light);
    let radial_bulge = smoothstep(0.05, 0.95, round_mask);
    let rim = pow(1.0 - abs(dot(curved_n, v)), 3.2);
    let body_t = clamp(mix(stepped, max(soft_band, radial_bulge * 0.9), 0.72), 0.0, 1.0);
    let body_rgb = mix(material.secondary_color.rgb, material.base_color.rgb, body_t);
    let outlined_rgb = mix(body_rgb, vec3(1.0), rim * material.rim_strength);
    return vec4(outlined_rgb, material.ghost_alpha);
}
