#import bevy_pbr::{
    pbr_types,
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
}
#import "shaders/shadow_style.wgsl"::apply_directional_shadow_style

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
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var terrain_feature_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var terrain_feature_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var terrain_macro_noise: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var terrain_macro_noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var terrain_macro_overlay: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var terrain_macro_overlay_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var river_flow_noise: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var river_flow_noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var terrain_feature_lut: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var terrain_feature_lut_sampler: sampler;

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

/// 低周波 2D ノイズ（各成分おおよそ −1..1）。
/// 複数の sine/cosine を重ねて有機的な低周波パターンを生成する。
fn lf_noise(p: vec2<f32>) -> vec2<f32> {
    let a = p.x * 1.7 + p.y * 0.9;
    let b = p.y * 1.3 + p.x * 0.8;
    return vec2(
        sin(a + 1.3) * 0.6 + sin(a * 0.5 + b * 0.7) * 0.4,
        cos(b + 2.1) * 0.6 + cos(b * 0.5 + a * 0.7) * 0.4,
    );
}

fn sample_macro_noise(world_xz: vec2<f32>, scale: f32) -> vec3<f32> {
    return textureSample(
        terrain_macro_noise,
        terrain_macro_noise_sampler,
        world_xz * scale,
    ).rgb * 2.0 - 1.0;
}

fn sample_macro_overlay(world_xz: vec2<f32>, scale: f32) -> f32 {
    return textureSample(
        terrain_macro_overlay,
        terrain_macro_overlay_sampler,
        world_xz * scale,
    ).r * 2.0 - 1.0;
}

fn sample_river_flow_noise(world_xz: vec2<f32>) -> f32 {
    return textureSample(
        river_flow_noise,
        river_flow_noise_sampler,
        vec2(
            world_xz.x * 0.0015 - globals.time * 0.05,
            world_xz.y * 0.0045,
        ),
    ).r * 2.0 - 1.0;
}

fn sample_feature_lut(idx: f32) -> vec4<f32> {
    let uv = vec2((idx + 0.5) / 256.0, 0.5);
    return textureSample(terrain_feature_lut, terrain_feature_lut_sampler, uv);
}

/// 地形フィーチャーマップをサンプル。
/// 戻り値: vec4(shore_sand, inland_sand, rock_field, zone_bias)（0..1）。
/// `map_world_width/height` が 0 のとき（建物など非地形）は vec4(0) を返す。
fn sample_feature(world_xz: vec2<f32>) -> vec4<f32> {
    if section_material.map_world_width <= 0.0 {
        return vec4(0.0);
    }
    // ワールド原点はマップ中心。feature map は左上原点の 0..1 UV なので、
    // half-extent を足してから正規化する。
    let half_size = vec2(
        section_material.map_world_width * 0.5,
        section_material.map_world_height * 0.5,
    );
    let uv = vec2(
        (world_xz.x + half_size.x) / section_material.map_world_width,
        (-world_xz.y + half_size.y) / section_material.map_world_height,
    );
    return textureSample(terrain_feature_map, terrain_feature_sampler, uv);
}

fn apply_feature_grade(base_rgb: vec3<f32>, lut: vec4<f32>, weight: f32, strength: f32) -> vec3<f32> {
    if weight <= 0.0 {
        return base_rgb;
    }

    let signed_tint = (lut.rgb - vec3(0.5)) * 2.0;
    let mul = clamp(vec3(1.0) + signed_tint * strength * 0.75, vec3(0.60), vec3(1.40));
    let add = signed_tint * strength * 0.16;
    let graded = clamp(base_rgb * mul + add, vec3(0.0), vec3(1.0));
    return mix(base_rgb, graded, clamp(weight, 0.0, 1.0));
}

fn apply_luma_normalized_tint(
    base_rgb: vec3<f32>,
    tint_rgb: vec3<f32>,
    weight: f32,
    strength: f32,
) -> vec3<f32> {
    if weight <= 0.0 {
        return base_rgb;
    }

    let w = clamp(weight, 0.0, 1.0);
    let luma_weights = vec3(0.299, 0.587, 0.114);
    let tint_luma = max(dot(tint_rgb, luma_weights), 0.001);
    let normalized_tint = clamp(tint_rgb / tint_luma, vec3(0.75), vec3(1.25));
    let tinted = clamp(base_rgb * mix(vec3(1.0), normalized_tint, strength), vec3(0.0), vec3(1.0));
    return mix(base_rgb, tinted, w);
}

fn apply_shoreline_tone(base_rgb: vec3<f32>, weight: f32) -> vec3<f32> {
    if weight <= 0.0 {
        return base_rgb;
    }

    let w = clamp(weight, 0.0, 1.0);
    let luma = dot(base_rgb, vec3(0.299, 0.587, 0.114));
    let muted = mix(base_rgb, vec3(luma), 0.18 * w);
    let shore_target = vec3(0.62, 0.61, 0.58);
    let toned = mix(muted, shore_target * (0.84 + luma * 0.16), 0.22 * w);
    return clamp(toned * vec3(0.98, 0.99, 1.00), vec3(0.0), vec3(1.0));
}

fn apply_palette_bias(
    base_rgb: vec3<f32>,
    target_rgb: vec3<f32>,
    weight: f32,
    strength: f32,
    chroma_boost: f32,
) -> vec3<f32> {
    if weight <= 0.0 {
        return base_rgb;
    }

    let w = clamp(weight, 0.0, 1.0);
    let luma_weights = vec3(0.299, 0.587, 0.114);
    let target_luma = max(dot(target_rgb, luma_weights), 0.001);
    let base_luma = dot(base_rgb, luma_weights);
    let normalized_tint = clamp(target_rgb / target_luma, vec3(0.72), vec3(1.32));

    var biased = base_rgb * mix(vec3(1.0), normalized_tint, strength);
    let biased_luma = max(dot(biased, luma_weights), 0.001);
    biased *= base_luma / biased_luma;

    let chroma = biased - vec3(base_luma);
    biased = clamp(vec3(base_luma) + chroma * (1.0 + chroma_boost * w), vec3(0.0), vec3(1.0));
    return mix(base_rgb, biased, w);
}

fn apply_single_zone_bias(
    base_rgb: vec3<f32>,
    zone_weight: f32,
    target_rgb: vec3<f32>,
    strength: f32,
    chroma_boost: f32,
) -> vec3<f32> {
    return apply_palette_bias(base_rgb, target_rgb, zone_weight, strength, chroma_boost);
}

fn apply_value_emphasis(
    base_rgb: vec3<f32>,
    weight: f32,
    darken_rgb: vec3<f32>,
    strength: f32,
) -> vec3<f32> {
    if weight <= 0.0 {
        return base_rgb;
    }

    return mix(base_rgb, base_rgb * darken_rgb, clamp(weight, 0.0, 1.0) * strength);
}

fn is_grass_material() -> bool {
    return section_material.terrain_kind < 0.5;
}

fn is_dirt_material() -> bool {
    return section_material.terrain_kind >= 0.5 && section_material.terrain_kind < 1.5;
}

fn is_sand_material() -> bool {
    return section_material.terrain_kind >= 1.5 && section_material.terrain_kind < 2.5;
}

fn grade_grass(base_rgb: vec3<f32>, brightness: f32, feature: vec4<f32>) -> vec3<f32> {
    let lit_rgb = base_rgb * brightness;
    let grass_zone = max((0.5 - feature.a) * 2.0, 0.0);
    return apply_single_zone_bias(lit_rgb, grass_zone, vec3(0.40, 0.56, 0.39), 0.42, 0.28);
}

fn grade_dirt(base_rgb: vec3<f32>, brightness: f32, feature: vec4<f32>) -> vec3<f32> {
    let rock_field = feature.b;
    let rock_lut = sample_feature_lut(3.0);
    let dirt_zone = max((feature.a - 0.5) * 2.0, 0.0);
    var graded_rgb = apply_single_zone_bias(
        base_rgb * brightness,
        dirt_zone,
        vec3(0.52, 0.39, 0.27),
        0.22,
        0.18,
    );
    graded_rgb = apply_value_emphasis(graded_rgb, dirt_zone, vec3(0.82, 0.78, 0.74), 0.42);
    graded_rgb = apply_feature_grade(graded_rgb, rock_lut, rock_field, 1.35);
    return graded_rgb;
}

fn grade_sand(base_rgb: vec3<f32>, brightness: f32, feature: vec4<f32>) -> vec3<f32> {
    let shore_sand = feature.r;
    let inland_sand = feature.g;
    let shore_lut = sample_feature_lut(1.0);
    let inland_lut = sample_feature_lut(2.0);
    var graded_rgb = base_rgb * brightness;
    graded_rgb = apply_feature_grade(graded_rgb, shore_lut, shore_sand, 1.05);
    graded_rgb = apply_feature_grade(graded_rgb, inland_lut, inland_sand, 0.90);
    graded_rgb = apply_shoreline_tone(graded_rgb, shore_sand * 0.95);
    return graded_rgb;
}

/// 地形タイル用 UV 計算（ワールド XZ をそのままスケール。タイル境界で連続）。
///
/// - domain warp: UV サンプル位置を低周波ノイズでずらし、繰り返し感を崩す
/// - uv_distort_strength: 草専用の高周波 UV 揺れ（後方互換）
/// - uv_scroll_speed > 0（川）: 横スクロール + 弱い flow distortion
fn compute_terrain_uv(world_xz: vec2<f32>) -> vec2<f32> {
    // domain warp: UV サンプル元座標をワールド低周波ノイズでずらす
    var sample_xz = world_xz;
    if section_material.domain_warp_strength > 0.0 {
        let warp = sample_macro_noise(world_xz, 0.00075).xy;
        sample_xz = world_xz + warp * section_material.domain_warp_strength;
    }

    let base_uv = sample_xz * section_material.uv_scale;
    var uv = base_uv;

    // 草専用 A3: UV 空間の低周波歪み（後方互換）
    if section_material.uv_distort_strength > 0.0 {
        let wobble = sin(world_xz * 0.05 + vec2<f32>(0.3, 1.7)) * section_material.uv_distort_strength;
        uv = base_uv + wobble;
    }

    // 川: flow distortion（V 軸へ弱いうねりを加算）
    if section_material.uv_scroll_speed > 0.0 {
        uv.y += sample_river_flow_noise(world_xz) * 0.03;
    }

    return uv + vec2<f32>(-section_material.uv_scroll_speed * globals.time, 0.0);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    section_discard(in.world_position.xyz);

    var in_mut = in;
    if section_material.albedo_uv_mode > 0.5 {
        in_mut.uv = compute_terrain_uv(in.world_position.xz);
    }

    var pbr_input = pbr_input_from_standard_material(in_mut, is_front);
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    if section_material.albedo_uv_mode > 0.5 {
        let world_xz = in.world_position.xz;

        // マクロ明度変調: 低周波ノイズで面全体に緩やかな明暗ムラを与える
        var brightness = 1.0;
        if section_material.brightness_variation_strength > 0.0 {
            let macro_noise = sample_macro_noise(world_xz, 0.00045).z;
            let overlay = sample_macro_overlay(world_xz, 0.0012);
            let mixed_noise = macro_noise * 0.35 + overlay * 0.65;
            brightness = 1.0 + mixed_noise * section_material.brightness_variation_strength * 1.8;
        }

        let feature = sample_feature(world_xz);
        let c = pbr_input.material.base_color;
        var graded_rgb = c.rgb * brightness;
        var roughness_delta = 0.0;
        if is_grass_material() {
            graded_rgb = grade_grass(c.rgb, brightness, feature);
        } else if is_dirt_material() {
            graded_rgb = grade_dirt(c.rgb, brightness, feature);
            let rock_lut = sample_feature_lut(3.0);
            roughness_delta = feature.b * (rock_lut.a - 0.5) * 0.4;
        } else if is_sand_material() {
            graded_rgb = grade_sand(c.rgb, brightness, feature);
            let shore_lut = sample_feature_lut(1.0);
            let inland_lut = sample_feature_lut(2.0);
            roughness_delta =
                feature.r * (shore_lut.a - 0.5) * 0.4
                + feature.g * (inland_lut.a - 0.5) * 0.4;
        }
        pbr_input.material.base_color = vec4<f32>(graded_rgb, c.a);
        pbr_input.material.perceptual_roughness = clamp(
            pbr_input.material.perceptual_roughness + roughness_delta,
            0.0,
            1.0,
        );
    }

    var out: FragmentOutput;
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
        out.color = vec4<f32>(
            apply_directional_shadow_style(
                pbr_input,
                out.color.rgb,
                section_material.shadow_style_params,
                section_material.shadow_style_tint,
                section_material.shadow_style_blur,
                section_material.soul_shadow_projectors,
                section_material.soul_shadow_projector_meta,
            ),
            out.color.a,
        );
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
