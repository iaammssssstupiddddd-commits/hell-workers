#import bevy_pbr::{
    pbr_prepass_functions,
    prepass_io,
    pbr_functions,
}

#import "shaders/terrain_surface_types.wgsl"::TerrainSurfaceUniforms

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
