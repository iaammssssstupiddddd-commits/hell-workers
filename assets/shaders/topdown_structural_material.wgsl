#import bevy_pbr::{
    pbr_types,
    mesh_view_bindings as view_bindings,
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}
#import "shaders/shadow_style.wgsl"::apply_directional_shadow_style
#import hell_workers::indoor_light_field::sample_indoor_light_field

struct TopDownStructuralUniforms {
    build_progress:     f32,
    wall_height:        f32,
    shadow_style_params: vec4<f32>,
    shadow_style_tint:   vec4<f32>,
    shadow_style_blur:   vec4<f32>,
    indoor_light_params: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> structural_material: TopDownStructuralUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var indoor_light_field: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var indoor_light_sampler: sampler;

fn structural_discard(world_position: vec3<f32>) {
    if structural_material.wall_height > 0.0 {
        let progress_boundary =
            structural_material.wall_height * structural_material.build_progress;
        if world_position.y > progress_boundary {
            discard;
        }
    }
}

// Albedo-first illustration: geometry still owns depth, silhouettes and shadow
// bias, but its normals no longer paint round/box-shaped gradients into the art.
fn illustrated_lighting(in: pbr_types::PbrInput) -> vec4<f32> {
    var paper = in;
    paper.N = vec3<f32>(0.0, 1.0, 0.0);
    paper.material.base_color = vec4<f32>(1.0);
    paper.material.emissive = vec4<f32>(0.0);
    paper.material.metallic = 0.0;
    paper.material.reflectance = vec3<f32>(0.0);
    paper.material.perceptual_roughness = 1.0;
    paper.diffuse_occlusion = vec3<f32>(1.0);
    paper.specular_occlusion = 1.0;
    let illumination = apply_pbr_lighting(paper).rgb;
    let luminance = dot(illumination, vec3<f32>(0.2126, 0.7152, 0.0722));
    // Four steps per linear light unit. Keep dim light continuous rather than
    // rounding it to black or introducing an unlit brightness floor.
    let band = select(luminance, floor(luminance * 4.0 + 0.5) * 0.25, luminance >= 0.25);
    let light_color = illumination / max(luminance, 0.00001);
    // Match Bevy's surface-emission exposure rule, exactly once and outside the
    // white reference, so active Spa slots retain their authored emission mask.
    let emission = in.material.emissive.rgb * in.material.base_color.a
        * mix(1.0, view_bindings::view.exposure, in.material.emissive.a);
    return vec4<f32>(in.material.base_color.rgb * light_color * band + emission,
                     in.material.base_color.a);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    structural_discard(in.world_position.xyz);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        if structural_material.indoor_light_params.w == 1.0 {
            out.color = illustrated_lighting(pbr_input);
        } else {
            out.color = apply_pbr_lighting(pbr_input);
            out.color = vec4<f32>(
                apply_directional_shadow_style(
                    pbr_input,
                    out.color.rgb,
                    structural_material.shadow_style_params,
                    structural_material.shadow_style_tint,
                    structural_material.shadow_style_blur,
                ),
                out.color.a,
            );
        }
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
