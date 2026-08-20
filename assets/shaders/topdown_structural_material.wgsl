#import bevy_pbr::{
    pbr_types,
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_functions,
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
        if structural_material.indoor_light_params.z > 0.5 {
            let local_light = sample_indoor_light_field(
                indoor_light_field,
                indoor_light_sampler,
                in.world_position.xyz,
                in.world_normal,
                mesh_functions::get_tag(in.instance_index),
                structural_material.indoor_light_params.x,
            ) * structural_material.indoor_light_params.y;
            out.color = vec4<f32>(
                out.color.rgb + pbr_input.material.base_color.rgb * local_light,
                out.color.a,
            );
        }
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
