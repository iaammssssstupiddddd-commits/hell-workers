#import bevy_pbr::{
    pbr_prepass_functions,
    prepass_io,
    pbr_functions,
}

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

fn structural_discard(world_position: vec3<f32>) {
    if structural_material.wall_height > 0.0 {
        let progress_boundary =
            structural_material.wall_height * structural_material.build_progress;
        if world_position.y > progress_boundary {
            discard;
        }
    }
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) _is_front: bool,
) -> prepass_io::FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
#endif
    structural_discard(in.world_position.xyz);
    pbr_prepass_functions::prepass_alpha_discard(in);

    var out: prepass_io::FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
    return out;
}
#endif // PREPASS_FRAGMENT
