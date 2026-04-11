#import bevy_pbr::forward_io::VertexOutput

struct SoulMaskMaterial {
    color: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: SoulMaskMaterial;

@fragment
fn fragment(_in: VertexOutput) -> @location(0) vec4<f32> {
    return material.color;
}
