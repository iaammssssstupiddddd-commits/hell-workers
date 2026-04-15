// terrain_surface_material_lod1_lite.wgsl
//
// LOD1-lite: LOD1 と LOD2 の中間品質。
//
// LOD1 との差異（削減項目）:
//   - compute_terrain_uv: domain warp・UV distort・river scroll を除去。UV は world_xz * uv_scale のみ。
//   - sample_surface_color_lod1_lite: macro-noise 明度変調・macro overlay を除去。brightness = 1.0 固定。
//   - grade_sand_lod1_lite: 砂浜の色合わせを維持するため shoreline tone は full 品質に合わせる。
//   - テクスチャサンプリング削減: macro_noise(111)・overlay(113/115/117)・flow_noise(119)・
//     normal_detail(121) の 5 テクスチャを参照しない。
//
// LOD2 との差異（維持項目）:
//   - blend_terrain_lod1_lite: 4-corner 手動 bilinear ブレンドを維持（LOD2 は nearest-only）。
//   - boundary_proximity_mask early-out を維持（内部タイルはバイリニア不要）。
//   - albedo UV 量子化なし（LOD2 の LOD2_ALBEDO_WORLD_TEXEL 処理なし）。
//
// バインドグループは 100〜132 が LOD1/LOD2 と同一。
// LOD1-lite で未参照のスロット（111〜124）はバインドされるが GPU からアクセスされない。

#import bevy_pbr::{
    pbr_types,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}
#import "shaders/shadow_style.wgsl" apply_directional_shadow_style

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
    shadow_style_params:        vec4<f32>,
    shadow_style_tint:          vec4<f32>,
    shadow_style_blur:          vec4<f32>,
    soul_shadow_projectors:     array<vec4<f32>, 12>,
    soul_shadow_projector_meta: vec4<f32>,
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
// binding 111〜126: LOD1 との AsBindGroup レイアウト一致のために宣言。LOD1-lite では未参照。
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var terrain_macro_noise: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var macro_noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(113) var grass_macro_overlay: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(114) var grass_overlay_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(115) var dirt_macro_overlay: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(116) var dirt_overlay_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(117) var sand_macro_overlay: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(118) var sand_overlay_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(119) var terrain_blend_mask_soft: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(120) var blend_mask_soft_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(121) var river_flow_noise: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(122) var river_flow_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(123) var river_normal_like: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(124) var river_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(125) var shoreline_detail: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(126) var shoreline_detail_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(127) var terrain_feature_lut: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(128) var feature_lut_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(129) var boundary_mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(130) var boundary_mask_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(131) var boundary_proximity_mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(132) var boundary_proximity_sampler: sampler;

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

fn apply_shoreline_tone(base_rgb: vec3<f32>, weight: f32, shoreline_detail_weight: f32) -> vec3<f32> {
    if weight <= 0.0 {
        return base_rgb;
    }

    let w = clamp(weight, 0.0, 1.0);
    let luma = dot(base_rgb, vec3(0.299, 0.587, 0.114));
    let muted = mix(base_rgb, vec3(luma), 0.18 * w);
    let shore_target = vec3(0.62, 0.61, 0.58);
    let detail_mul = 1.0 + (shoreline_detail_weight - 0.5) * 0.12;
    let toned = mix(muted, shore_target * (0.84 + luma * 0.16), 0.22 * w);
    return clamp(toned * vec3(0.98, 0.99, 1.00) * detail_mul, vec3(0.0), vec3(1.0));
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

fn grade_grass_lod1_lite(base_rgb: vec3<f32>, feature: vec4<f32>) -> vec3<f32> {
    let grass_zone = max((0.5 - feature.a) * 2.0, 0.0);
    return apply_palette_bias(base_rgb, vec3(0.40, 0.56, 0.39), grass_zone, 0.42, 0.28);
}

fn grade_dirt_lod1_lite(base_rgb: vec3<f32>, feature: vec4<f32>) -> vec3<f32> {
    let rock_field = feature.b;
    let rock_lut = sample_feature_lut(3.0);
    let dirt_zone = max((feature.a - 0.5) * 2.0, 0.0);
    var graded_rgb = apply_palette_bias(base_rgb, vec3(0.52, 0.39, 0.27), dirt_zone, 0.22, 0.18);
    graded_rgb = apply_value_emphasis(graded_rgb, dirt_zone, vec3(0.82, 0.78, 0.74), 0.42);
    graded_rgb = apply_feature_grade(graded_rgb, rock_lut, rock_field, 1.35);
    return graded_rgb;
}

// LOD1-lite 版: 砂浜色の段差を避けるため shoreline_tone は full と揃える。
fn grade_sand_lod1_lite(base_rgb: vec3<f32>, feature: vec4<f32>, world_xz: vec2<f32>) -> vec3<f32> {
    let shore_sand  = feature.r;
    let inland_sand = feature.g;
    let shore_lut   = sample_feature_lut(1.0);
    let inland_lut  = sample_feature_lut(2.0);
    let shoreline_detail_weight = textureSample(
        shoreline_detail,
        shoreline_detail_sampler,
        world_xz * 0.0025,
    ).r;
    var graded_rgb = base_rgb;
    graded_rgb = apply_feature_grade(graded_rgb, shore_lut, shore_sand,  1.05);
    graded_rgb = apply_feature_grade(graded_rgb, inland_lut, inland_sand, 0.90);
    graded_rgb = apply_shoreline_tone(graded_rgb, shore_sand * 0.95, shoreline_detail_weight);
    return graded_rgb;
}

// LOD1-lite 版: domain warp・UV distort・river scroll なし。UV は world_xz * uv_scale のみ。
// macro-noise 明度変調・macro overlay も除去。brightness = 1.0 固定。
fn sample_surface_color_lod1_lite(
    id: u32,
    world_xz: vec2<f32>,
    feature: vec4<f32>,
    raw_byte: u32,
) -> vec3<f32> {
    let uv = world_xz * tsm.uv_scale;
    let base_rgb = sample_albedo(id, uv);
    let vmul = variant_luma_mul(raw_byte, id);
    switch id {
        case 0u: { return grade_grass_lod1_lite(base_rgb * vmul, feature); }
        case 1u: { return grade_dirt_lod1_lite(base_rgb * vmul,  feature); }
        case 2u: { return grade_sand_lod1_lite(base_rgb, feature, world_xz); }
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

// LOD1-lite 版: 4-corner 手動 bilinear ブレンドを維持しつつ、
// sample_surface_color の代わりに sample_surface_color_lod1_lite を使用。
// boundary_proximity_mask early-out も維持（内部タイルはバイリニア不要）。
fn blend_terrain_lod1_lite(world_xz: vec2<f32>, cell: vec2<i32>, feature_in: vec4<f32>) -> vec3<f32> {
    let uv = world_to_boundary_uv(world_xz);

    // Early-out: 境界近傍マスクが 0（内部タイル）ならバイリニア不要。
    let is_boundary = textureSample(boundary_proximity_mask, boundary_proximity_sampler, uv).r > 0.5;
    if !is_boundary {
        let raw_center_fast = textureSample(boundary_mask, boundary_mask_sampler, uv).r;
        let region_id_fast  = region_to_coarse_id(raw_center_fast);
        let region_raw_fast = region_to_raw_byte(raw_center_fast);
        let eff_f = feature_with_zone_tone(feature_in, region_raw_fast, region_id_fast);
        return sample_surface_color_lod1_lite(region_id_fast, world_xz, eff_f, region_raw_fast);
    }

    let dims = vec2<f32>(textureDimensions(boundary_mask, 0));

    let raw_center = textureSample(boundary_mask, boundary_mask_sampler, uv).r;
    let region_id  = region_to_coarse_id(raw_center);
    let region_raw = region_to_raw_byte(raw_center);

    var feature: vec4<f32> = feature_in;
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

    let tc   = uv * dims - 0.5;
    let tci  = floor(tc);
    let frac = tc - tci;

    let uv00 = clamp((tci + vec2(0.5, 0.5)) / dims, vec2(0.0), vec2(1.0));
    let uv10 = clamp((tci + vec2(1.5, 0.5)) / dims, vec2(0.0), vec2(1.0));
    let uv01 = clamp((tci + vec2(0.5, 1.5)) / dims, vec2(0.0), vec2(1.0));
    let uv11 = clamp((tci + vec2(1.5, 1.5)) / dims, vec2(0.0), vec2(1.0));

    let raw00 = textureSample(boundary_mask, boundary_mask_sampler, uv00).r;
    let raw10 = textureSample(boundary_mask, boundary_mask_sampler, uv10).r;
    let raw01 = textureSample(boundary_mask, boundary_mask_sampler, uv01).r;
    let raw11 = textureSample(boundary_mask, boundary_mask_sampler, uv11).r;

    let id00 = region_to_coarse_id(raw00);
    let id10 = region_to_coarse_id(raw10);
    let id01 = region_to_coarse_id(raw01);
    let id11 = region_to_coarse_id(raw11);

    let braw00 = region_to_raw_byte(raw00);
    let braw10 = region_to_raw_byte(raw10);
    let braw01 = region_to_raw_byte(raw01);
    let braw11 = region_to_raw_byte(raw11);

    var base_color: vec3<f32>;
    if id00 == id10 && id00 == id01 && id00 == id11
       && braw00 == braw10 && braw00 == braw01 && braw00 == braw11 {
        let eff_f = feature_with_zone_tone(feature, region_raw, region_id);
        base_color = sample_surface_color_lod1_lite(id00, world_xz, eff_f, region_raw);
    } else {
        var other_id: u32 = region_id;
        if      id00 != region_id { other_id = id00; }
        else if id10 != region_id { other_id = id10; }
        else if id01 != region_id { other_id = id01; }
        else if id11 != region_id { other_id = id11; }

        var feature_other: vec4<f32> = feature_in;
        if other_id != region_id && region_id == cell_terrain_id(cell) {
            let n0 = clamp_cell(cell + vec2<i32>(-1,  0));
            let n1 = clamp_cell(cell + vec2<i32>( 1,  0));
            let n2 = clamp_cell(cell + vec2<i32>( 0, -1));
            let n3 = clamp_cell(cell + vec2<i32>( 0,  1));
            let n4 = clamp_cell(cell + vec2<i32>(-1, -1));
            let n5 = clamp_cell(cell + vec2<i32>( 1, -1));
            let n6 = clamp_cell(cell + vec2<i32>(-1,  1));
            let n7 = clamp_cell(cell + vec2<i32>( 1,  1));
            if      cell_terrain_id(n0) == other_id { feature_other = sample_feature(n0); }
            else if cell_terrain_id(n1) == other_id { feature_other = sample_feature(n1); }
            else if cell_terrain_id(n2) == other_id { feature_other = sample_feature(n2); }
            else if cell_terrain_id(n3) == other_id { feature_other = sample_feature(n3); }
            else if cell_terrain_id(n4) == other_id { feature_other = sample_feature(n4); }
            else if cell_terrain_id(n5) == other_id { feature_other = sample_feature(n5); }
            else if cell_terrain_id(n6) == other_id { feature_other = sample_feature(n6); }
            else if cell_terrain_id(n7) == other_id { feature_other = sample_feature(n7); }
        }
        let f00 = feature_with_zone_tone(select(feature_other, feature, id00 == region_id), braw00, id00);
        let f10 = feature_with_zone_tone(select(feature_other, feature, id10 == region_id), braw10, id10);
        let f01 = feature_with_zone_tone(select(feature_other, feature, id01 == region_id), braw01, id01);
        let f11 = feature_with_zone_tone(select(feature_other, feature, id11 == region_id), braw11, id11);
        let c00 = sample_surface_color_lod1_lite(id00, world_xz, f00, braw00);
        let c10 = sample_surface_color_lod1_lite(id10, world_xz, f10, braw10);
        let c01 = sample_surface_color_lod1_lite(id01, world_xz, f01, braw01);
        let c11 = sample_surface_color_lod1_lite(id11, world_xz, f11, braw11);
        base_color = mix(mix(c00, c10, frac.x), mix(c01, c11, frac.x), frac.y);
    }

    return base_color;
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
    let graded_rgb = blend_terrain_lod1_lite(world_xz, cell, feature);
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
        out.color = vec4<f32>(
            apply_directional_shadow_style(
                pbr_input,
                out.color.rgb,
                tsm.shadow_style_params,
                tsm.shadow_style_tint,
                tsm.shadow_style_blur,
                tsm.soul_shadow_projectors,
                tsm.soul_shadow_projector_meta,
            ),
            out.color.a,
        );
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
