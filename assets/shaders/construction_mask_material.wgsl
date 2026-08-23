#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_functions::get_tag,
}

const MASK_PROGRESS_BITS: u32 = 0xffu;
const MASK_STATE_SHIFT: u32 = 8u;
const MASK_STATE_BITS: u32 = 0x0fu;
const MASK_KIND_WALL_BIT: u32 = 1u << 12u;

fn srgb_component_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_component_to_linear(color.r),
        srgb_component_to_linear(color.g),
        srgb_component_to_linear(color.b),
    );
}

fn floor_mask_color(state: u32, progress: f32) -> vec4<f32> {
    switch state {
        case 0u: { return vec4<f32>(0.50, 0.50, 0.80, 0.20); }
        case 1u: { return vec4<f32>(0.65, 0.65, 0.90, 0.35); }
        case 2u: {
            return vec4<f32>(
                0.60 + 0.18 * progress,
                0.58 + 0.14 * progress,
                0.52 + 0.10 * progress,
                0.35 + 0.25 * progress,
            );
        }
        case 3u: { return vec4<f32>(0.78, 0.72, 0.60, 0.60); }
        case 4u: { return vec4<f32>(0.55, 0.44, 0.34, 0.30); }
        case 5u: { return vec4<f32>(0.60, 0.48, 0.36, 0.45); }
        case 6u: {
            return vec4<f32>(
                0.52 - 0.18 * progress,
                0.44 - 0.14 * progress,
                0.34 - 0.10 * progress,
                0.50 + 0.40 * progress,
            );
        }
        default: { return vec4<f32>(0.33, 0.33, 0.35, 0.95); }
    }
}

fn wall_mask_color(state: u32, progress: f32) -> vec4<f32> {
    switch state {
        case 0u: { return vec4<f32>(0.78, 0.56, 0.32, 0.25); }
        case 1u: { return vec4<f32>(0.90, 0.68, 0.36, 0.40); }
        case 2u: {
            return vec4<f32>(
                0.86 - 0.20 * progress,
                0.66 - 0.20 * progress,
                0.38 - 0.12 * progress,
                0.40 + 0.35 * progress,
            );
        }
        case 3u: { return vec4<f32>(0.58, 0.42, 0.30, 0.70); }
        case 4u: { return vec4<f32>(0.55, 0.44, 0.34, 0.30); }
        case 5u: { return vec4<f32>(0.62, 0.50, 0.37, 0.45); }
        case 6u: {
            return vec4<f32>(
                0.56 - 0.22 * progress,
                0.46 - 0.18 * progress,
                0.35 - 0.11 * progress,
                0.50 + 0.42 * progress,
            );
        }
        default: { return vec4<f32>(0.35, 0.35, 0.38, 0.95); }
    }
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let tag = get_tag(in.instance_index);
    let state = (tag >> MASK_STATE_SHIFT) & MASK_STATE_BITS;
    let progress = f32(tag & MASK_PROGRESS_BITS) / 100.0;
    let is_wall = (tag & MASK_KIND_WALL_BIT) != 0u;
    let srgb = select(
        floor_mask_color(state, progress),
        wall_mask_color(state, progress),
        is_wall,
    );

    var out: FragmentOutput;
    out.color = vec4<f32>(srgb_to_linear(srgb.rgb), srgb.a);
    return out;
}
