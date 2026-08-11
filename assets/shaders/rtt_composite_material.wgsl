#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(1) var scene_texture: texture_2d<f32>;
@group(2) @binding(2) var scene_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(scene_texture, scene_sampler, in.uv);
}
