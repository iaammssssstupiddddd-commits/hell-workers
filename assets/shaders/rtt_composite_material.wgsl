#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct RttCompositeMaterial {
    pixel_size: vec2<f32>,
    mask_radius_px: f32,
    mask_feather: f32,
}

@group(2) @binding(0) var<uniform> material: RttCompositeMaterial;
@group(2) @binding(1) var scene_texture: texture_2d<f32>;
@group(2) @binding(2) var scene_sampler: sampler;
@group(2) @binding(3) var soul_mask_texture: texture_2d<f32>;
@group(2) @binding(4) var soul_mask_sampler: sampler;

const SAMPLE_COUNT: u32 = 12u;
const SAMPLE_DIRS: array<vec2<f32>, 12> = array<vec2<f32>, 12>(
    vec2<f32>( 1.0,  0.0),
    vec2<f32>( 0.8660254,  0.5),
    vec2<f32>( 0.5,  0.8660254),
    vec2<f32>( 0.0,  1.0),
    vec2<f32>(-0.5,  0.8660254),
    vec2<f32>(-0.8660254,  0.5),
    vec2<f32>(-1.0,  0.0),
    vec2<f32>(-0.8660254, -0.5),
    vec2<f32>(-0.5, -0.8660254),
    vec2<f32>( 0.0, -1.0),
    vec2<f32>( 0.5, -0.8660254),
    vec2<f32>( 0.8660254, -0.5),
);

fn soul_mask_alpha(uv: vec2<f32>) -> f32 {
    return textureSample(soul_mask_texture, soul_mask_sampler, uv).a;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let scene = textureSample(scene_texture, scene_sampler, uv);
    let center_mask = soul_mask_alpha(uv);
    let radius = material.pixel_size * material.mask_radius_px;

    var mask_sum = center_mask;
    var mask_max = center_mask;
    var color_sum = scene.rgb * center_mask;
    var color_weight = center_mask;

    for (var i = 0u; i < SAMPLE_COUNT; i = i + 1u) {
        let sample_uv = uv + SAMPLE_DIRS[i] * radius;
        let sample_mask = soul_mask_alpha(sample_uv);
        let sample_scene = textureSample(scene_texture, scene_sampler, sample_uv);
        mask_sum += sample_mask;
        mask_max = max(mask_max, sample_mask);
        color_sum += sample_scene.rgb * sample_mask;
        color_weight += sample_mask;
    }

    let avg_mask = mask_sum / f32(SAMPLE_COUNT + 1u);
    let rounded_mask = max(
        center_mask,
        smoothstep(material.mask_feather, 0.92, mix(avg_mask, mask_max, 0.55)),
    );
    let expanded_rgb = select(scene.rgb, color_sum / color_weight, color_weight > 0.0001);
    let extra_alpha = clamp(rounded_mask - scene.a, 0.0, 1.0 - scene.a);
    let composed_rgb = mix(scene.rgb, expanded_rgb, extra_alpha);

    return vec4<f32>(composed_rgb, max(scene.a, rounded_mask));
}
