#import bevy_pbr::{
    pbr_prepass_functions,
    prepass_io,
    pbr_functions,
}

struct SectionMaterialUniforms {
    cut_position:    vec4<f32>,
    cut_normal:      vec4<f32>,
    thickness:       f32,
    cut_active:      f32,
    build_progress:  f32,
    wall_height:     f32,
    albedo_uv_mode:  f32,
    uv_scale:        f32,
    uv_scroll_speed:               f32,
    uv_distort_strength:           f32,
    brightness_variation_strength: f32,
    map_world_width:               f32,
    map_world_height:              f32,
    domain_warp_strength:          f32,
    terrain_kind:                  f32,
    shadow_style_params:           vec4<f32>,
    shadow_style_tint:             vec4<f32>,
    shadow_style_blur:             vec4<f32>,
    soul_shadow_projectors:        array<vec4<f32>, 12>,
    soul_shadow_projector_meta:    vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> section_material: SectionMaterialUniforms;

fn section_discard(world_position: vec3<f32>) {
    if section_material.cut_active > 0.5 {
        let dist = dot(
            world_position - section_material.cut_position.xyz,
            section_material.cut_normal.xyz
        );
        if dist < 0.0 || dist > section_material.thickness {
            discard;
        }
    }

    if section_material.wall_height > 0.0 {
        let progress_boundary = section_material.wall_height * section_material.build_progress;
        if world_position.y > progress_boundary {
            discard;
        }
    }
}

@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) _is_front: bool,
) -> prepass_io::FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
#endif
    section_discard(in.world_position.xyz);
    pbr_prepass_functions::prepass_alpha_discard(in);

    var out: prepass_io::FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
    return out;
}
