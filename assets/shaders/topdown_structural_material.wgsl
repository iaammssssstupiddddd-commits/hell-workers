#import bevy_pbr::{
    pbr_types,
    mesh_functions::get_world_from_local,
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

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    structural_discard(in.world_position.xyz);

    let projected_preview = structural_material.indoor_light_params.w == 3.0;
    let projected = structural_material.indoor_light_params.w == 2.0 || projected_preview;
    let uv_ink = structural_material.indoor_light_params.w == 4.0;
    var paint_in = in;
#ifdef VERTEX_UVS_A
    if projected {
        // This pilot is admitted only for the body part at the Root origin.
        // Zoom and owner translation cancel; geometry still owns depth/shadows.
        let model = get_world_from_local(in.instance_index);
        let scale = max(length(model[0].xyz), 0.00001);
        let relative = (in.world_position.xyz - model[3].xyz) / scale;
        var plane = (view_bindings::view.view_from_world * vec4<f32>(relative, 0.0)).xy;
        if projected_preview {
            // world_preview uses simulation-plane units, including the RtT
            // composite's vertical compensation. Apply that factor once here.
            let camera = view_bindings::view.view_from_world;
            let ground_gain = length(vec2<f32>(camera[0].y, camera[2].y));
            plane.y /= max(ground_gain, 0.00001);
        }
        let projection = structural_material.shadow_style_params;
        paint_in.uv = projection.zw + vec2<f32>(plane.x, -plane.y) / projection.xy;
    }
#endif
    var pbr_input = pbr_input_from_standard_material(paint_in, is_front);
    if projected {
        // Paint alpha describes coverage of the drawing, never geometry holes.
        // Composite on an explicit wood matte before alpha-discard/post processing.
        let paint = pbr_input.material.base_color;
        pbr_input.material.base_color = vec4<f32>(
            mix(structural_material.shadow_style_tint.rgb, paint.rgb, paint.a), 1.0);
    }
    // Keep real light/shadow response while reducing the faceted, cylindrical
    // shading on the illustrated Tank. Other structural materials retain zero.
    // Only the diffuse-lighting reference normal changes. The actual normal
    // still owns depth, shadow casting and receiver bias; UV ink follows mesh motion.
    let flatten = select(clamp(structural_material.indoor_light_params.w, 0.0, 1.0), 1.0, projected || uv_ink);
    pbr_input.N = normalize(mix(pbr_input.N, vec3<f32>(0.0, 1.0, 0.0), flatten));
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
        if !projected && !uv_ink {
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
