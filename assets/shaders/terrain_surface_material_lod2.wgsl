// terrain_surface_material_lod2.wgsl
//
// LOD2: chunk 維持のまま fragment コストを削減した簡略版シェーダー。
//
// LOD1 との差異:
//   - blend_terrain_lod2: 4-corner bilinear を廃止し nearest-only に。
//   - ただし `boundary_mask` の region を正本にし、曲線境界自体は維持する。
//   - sample_surface_color_lod2: macro-noise 明度変調・domain warp・UV distort・river scroll を除去。
//   - さらに albedo UV を量子化し、低解像度テクスチャ相当の見た目に落とす。
//   - grade_sand_lod2: shoreline_detail テクスチャサンプリングを除去。
//   - section_discard / PBR ライティング経路は LOD1 と同一。
//
// バインドグループはバインディング番号 100〜130 が LOD1 と同一。
// LOD2 シェーダーで未参照のスロット（111〜126）はバインドされるが GPU からアクセスされない。

#import bevy_pbr::{
    pbr_types,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}

struct TerrainSurfaceUniforms {
    cut_position:               vec4<f32>,
    cut_normal:                 vec4<f32>,
    thickness:                  f32,
    cut_active:                 f32,
    map_world_width:            f32,
    map_world_height:           f32,
    uv_scale:                   f32,
    blend_strength:             f32,
    macro_noise_scale:          f32,
    overlay_scale:              f32,
    lut_shore:                  vec4<f32>,
    lut_inland:                 vec4<f32>,
    lut_rock:                   vec4<f32>,
    feature_lut_constants_ready: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> tsm: TerrainSurfaceUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var terrain_id_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var terrain_feature_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var grass_albedo: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var grass_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var dirt_albedo: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var dirt_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var sand_albedo: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var sand_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var river_albedo: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var river_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(127) var terrain_feature_lut: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(128) var feature_lut_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(129) var boundary_mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(130) var boundary_mask_sampler: sampler;
// binding 131/132: boundary_proximity_mask（LOD1 early-out 用）。
// LOD2 では使用しないが、バインドグループレイアウトを LOD1/LOD1-lite と一致させるために宣言する。
@group(#{MATERIAL_BIND_GROUP}) @binding(131) var boundary_proximity_mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(132) var boundary_proximity_sampler: sampler;

const LOD2_ALBEDO_WORLD_TEXEL: f32 = 8.0;

fn section_discard(world_position: vec3<f32>) {
    if tsm.cut_active > 0.5 {
        let dist = dot(world_position - tsm.cut_position.xyz, tsm.cut_normal.xyz);
        if dist < 0.0 || dist > tsm.thickness {
            discard;
        }
    }
}

fn tile_size() -> f32 {
    return 1.0 / tsm.uv_scale;
}

fn world_to_cell(world_xz: vec2<f32>) -> vec2<i32> {
    let half_w = tsm.map_world_width * 0.5;
    let half_h = tsm.map_world_height * 0.5;
    let tile = tile_size();
    let map_w = i32(round(tsm.map_world_width / tile));
    let map_h = i32(round(tsm.map_world_height / tile));
    let ix = clamp(i32(floor((world_xz.x + half_w) / tile)), 0, map_w - 1);
    let iy = clamp(i32(floor((-world_xz.y + half_h) / tile)), 0, map_h - 1);
    return vec2(ix, iy);
}

fn world_to_boundary_uv(world_xz: vec2<f32>) -> vec2<f32> {
    return vec2(
        (world_xz.x + tsm.map_world_width  * 0.5) / tsm.map_world_width,
        (-world_xz.y + tsm.map_world_height * 0.5) / tsm.map_world_height,
    );
}

fn clamp_cell(cell: vec2<i32>) -> vec2<i32> {
    let tile = tile_size();
    let map_w = i32(round(tsm.map_world_width / tile));
    let map_h = i32(round(tsm.map_world_height / tile));
    return vec2<i32>(
        clamp(cell.x, 0, map_w - 1),
        clamp(cell.y, 0, map_h - 1),
    );
}

fn cell_terrain_id(cell: vec2<i32>) -> u32 {
    let raw = textureLoad(terrain_id_map, clamp_cell(cell), 0).r;
    return u32(round(raw * 3.0));
}

fn quantize_lod2_surface_world_xz(world_xz: vec2<f32>) -> vec2<f32> {
    let step = vec2<f32>(LOD2_ALBEDO_WORLD_TEXEL);
    return (floor(world_xz / step) + vec2<f32>(0.5)) * step;
}

fn sample_feature(cell: vec2<i32>) -> vec4<f32> {
    return textureLoad(terrain_feature_map, clamp_cell(cell), 0);
}

fn sample_feature_lut(idx: f32) -> vec4<f32> {
    if tsm.feature_lut_constants_ready > 0.5 {
        switch u32(round(idx)) {
            case 1u: { return tsm.lut_shore; }
            case 2u: { return tsm.lut_inland; }
            case 3u: { return tsm.lut_rock; }
            default: {}
        }
    }
    let uv = vec2((idx + 0.5) / 256.0, 0.5);
    return textureSample(terrain_feature_lut, feature_lut_sampler, uv);
}

fn sample_albedo(id: u32, uv: vec2<f32>) -> vec3<f32> {
    switch id {
        case 1u: { return textureSample(dirt_albedo, dirt_sampler, uv).rgb; }
        case 2u: { return textureSample(sand_albedo, sand_sampler, uv).rgb; }
        case 3u: { return textureSample(river_albedo, river_sampler, uv).rgb; }
        default: { return textureSample(grass_albedo, grass_sampler, uv).rgb; }
    }
}

fn variant_luma_mul(raw_byte: u32, coarse_id: u32) -> f32 {
    if coarse_id == 0u {
        switch raw_byte {
            case 0u: { return 1.0; }
            case 1u: { return 1.08; }
            case 2u: { return 0.92; }
            default: { return 1.0; }
        }
    }
    if coarse_id == 1u {
        switch raw_byte {
            case 85u: { return 1.0; }
            case 86u: { return 1.06; }
            case 87u: { return 0.88; }
            default: { return 1.0; }
        }
    }
    return 1.0;
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

fn region_to_coarse_id(raw: f32) -> u32 {
    return u32(round(raw * 3.0));
}

fn region_to_raw_byte(raw: f32) -> u32 {
    return u32(clamp(round(raw * 255.0), 0.0, 255.0));
}

fn feature_with_zone_tone(f: vec4<f32>, raw_byte: u32, terrain_id: u32) -> vec4<f32> {
    var result = f;
    if terrain_id == 0u {
        switch raw_byte {
            case 0u: { result.a = 0.0; }
            case 1u: { result.a = 0.5; }
            case 2u: { result.a = 1.0; }
            default: {}
        }
    } else if terrain_id == 1u {
        switch raw_byte {
            case 85u: { result.a = 0.0; }
            case 86u: { result.a = 0.5; }
            case 87u: { result.a = 1.0; }
            default:  {}
        }
    }
    return result;
}

fn grade_grass(base_rgb: vec3<f32>, brightness: f32, feature: vec4<f32>) -> vec3<f32> {
    let lit_rgb = base_rgb * brightness;
    let grass_zone = max((0.5 - feature.a) * 2.0, 0.0);
    return apply_palette_bias(lit_rgb, vec3(0.40, 0.56, 0.39), grass_zone, 0.42, 0.28);
}

fn grade_dirt(base_rgb: vec3<f32>, brightness: f32, feature: vec4<f32>) -> vec3<f32> {
    let rock_field = feature.b;
    let rock_lut = sample_feature_lut(3.0);
    let dirt_zone = max((feature.a - 0.5) * 2.0, 0.0);
    var graded_rgb = apply_palette_bias(
        base_rgb * brightness,
        vec3(0.52, 0.39, 0.27),
        dirt_zone,
        0.22,
        0.18,
    );
    graded_rgb = apply_value_emphasis(graded_rgb, dirt_zone, vec3(0.82, 0.78, 0.74), 0.42);
    graded_rgb = apply_feature_grade(graded_rgb, rock_lut, rock_field, 1.35);
    return graded_rgb;
}

/// LOD2 版: `shoreline_detail` テクスチャサンプリングを除去した簡略 grade_sand。
fn grade_sand_lod2(base_rgb: vec3<f32>, feature: vec4<f32>) -> vec3<f32> {
    let shore_sand  = feature.r;
    let inland_sand = feature.g;
    let shore_lut   = sample_feature_lut(1.0);
    let inland_lut  = sample_feature_lut(2.0);
    var graded_rgb = base_rgb;
    graded_rgb = apply_feature_grade(graded_rgb, shore_lut, shore_sand,  1.05);
    graded_rgb = apply_feature_grade(graded_rgb, inland_lut, inland_sand, 0.90);
    return graded_rgb;
}

/// LOD2 版: static UV（domain warp / distort / scroll なし）、明度変調なし。
/// さらに world-space UV を量子化し、遠景では低解像度 texture 相当の見た目に落とす。
fn sample_surface_color_lod2(
    id: u32,
    world_xz: vec2<f32>,
    feature: vec4<f32>,
    raw_byte: u32,
) -> vec3<f32> {
    let uv      = quantize_lod2_surface_world_xz(world_xz) * tsm.uv_scale;
    let base_rgb = sample_albedo(id, uv);
    let vmul    = variant_luma_mul(raw_byte, id);
    switch id {
        case 0u: { return grade_grass(base_rgb * vmul, 1.0, feature); }
        case 1u: { return grade_dirt(base_rgb * vmul,  1.0, feature); }
        case 2u: { return grade_sand_lod2(base_rgb, feature); }
        default: { return base_rgb; }
    }
}

fn roughness_delta_for_id(id: u32, feature: vec4<f32>) -> f32 {
    if id == 1u {
        let rock_lut = sample_feature_lut(3.0);
        return feature.b * (rock_lut.a - 0.5) * 0.4;
    }
    if id == 2u {
        let shore_lut = sample_feature_lut(1.0);
        let inland_lut = sample_feature_lut(2.0);
        return feature.r * (shore_lut.a - 0.5) * 0.4
            + feature.g * (inland_lut.a - 0.5) * 0.4;
    }
    return 0.0;
}

/// LOD2 版: nearest-only。4-corner bilinear ブレンドは除去する。
///
/// ただし曲線境界を消さないため、描画する coarse terrain 自体は `boundary_mask`
/// の nearest region を正本にする。feature は一致する terrain を近傍から探して補う。
fn blend_terrain_lod2(world_xz: vec2<f32>, cell: vec2<i32>, feature_in: vec4<f32>) -> vec3<f32> {
    let uv         = world_to_boundary_uv(world_xz);
    let raw_center = textureSample(boundary_mask, boundary_mask_sampler, uv).r;
    let region_id  = region_to_coarse_id(raw_center);
    let region_raw = region_to_raw_byte(raw_center);

    var feature = feature_in;
    if region_id != cell_terrain_id(cell) {
        let n0 = clamp_cell(cell + vec2<i32>(-1,  0));
        let n1 = clamp_cell(cell + vec2<i32>( 1,  0));
        let n2 = clamp_cell(cell + vec2<i32>( 0, -1));
        let n3 = clamp_cell(cell + vec2<i32>( 0,  1));
        let n4 = clamp_cell(cell + vec2<i32>(-1, -1));
        let n5 = clamp_cell(cell + vec2<i32>( 1, -1));
        let n6 = clamp_cell(cell + vec2<i32>(-1,  1));
        let n7 = clamp_cell(cell + vec2<i32>( 1,  1));
        if      cell_terrain_id(n0) == region_id { feature = sample_feature(n0); }
        else if cell_terrain_id(n1) == region_id { feature = sample_feature(n1); }
        else if cell_terrain_id(n2) == region_id { feature = sample_feature(n2); }
        else if cell_terrain_id(n3) == region_id { feature = sample_feature(n3); }
        else if cell_terrain_id(n4) == region_id { feature = sample_feature(n4); }
        else if cell_terrain_id(n5) == region_id { feature = sample_feature(n5); }
        else if cell_terrain_id(n6) == region_id { feature = sample_feature(n6); }
        else if cell_terrain_id(n7) == region_id { feature = sample_feature(n7); }
    }

    let eff_f = feature_with_zone_tone(feature, region_raw, region_id);
    return sample_surface_color_lod2(region_id, world_xz, eff_f, region_raw);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    section_discard(in.world_position.xyz);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    let world_xz = in.world_position.xz;
    let cell     = world_to_cell(world_xz);
    let center_id = cell_terrain_id(cell);
    let feature  = sample_feature(cell);
    let graded_rgb = blend_terrain_lod2(world_xz, cell, feature);
    let roughness_delta = roughness_delta_for_id(center_id, feature);

    pbr_input.material.base_color = vec4<f32>(graded_rgb, pbr_input.material.base_color.a);
    pbr_input.material.perceptual_roughness = clamp(
        pbr_input.material.perceptual_roughness + roughness_delta,
        0.0,
        1.0,
    );

    var out: FragmentOutput;
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
