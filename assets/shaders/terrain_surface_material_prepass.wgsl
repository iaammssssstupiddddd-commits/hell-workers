#import bevy_pbr::{
    pbr_prepass_functions,
    prepass_io,
    pbr_functions,
}

struct TerrainSurfaceUniforms {
    map_world_width:   f32,
    map_world_height:  f32,
    uv_scale:          f32,
    blend_strength:    f32,
    macro_noise_scale: f32,
    overlay_scale:     f32,
    lut_shore:         vec4<f32>,
    lut_inland:        vec4<f32>,
    lut_rock:          vec4<f32>,
    feature_lut_constants_ready: f32,
    shadow_style_params: vec4<f32>,
    shadow_style_tint:  vec4<f32>,
    shadow_style_blur:  vec4<f32>,
    indoor_light_params: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> terrain_surface: TerrainSurfaceUniforms;

@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) _is_front: bool,
) -> prepass_io::FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
#endif
    pbr_prepass_functions::prepass_alpha_discard(in);

    var out: prepass_io::FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
    return out;
}
