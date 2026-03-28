#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}

@fragment
fn fragment(_in: VertexOutput) -> FragmentOutput {
    discard;

    var out: FragmentOutput;
    out.color = vec4<f32>(0.0);
    return out;
}
