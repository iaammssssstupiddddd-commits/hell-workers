#import bevy_pbr::prepass_io

@fragment
fn fragment(in: prepass_io::VertexOutput) -> prepass_io::FragmentOutput {
    var out: prepass_io::FragmentOutput;

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif

    return out;
}
