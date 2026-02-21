// Dream泡パーティクル用フラグメントシェーダー（World空間 / Mesh2d用）
// ソフトグロー + 虹色屈折 + スペキュラハイライト + リム発光 + ノイズ変形

#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
}

struct DreamBubbleMaterial {
    color: vec4<f32>,  // offset 0:  ベース色 (16 bytes)
    time: f32,         // offset 16: 経過時間
    alpha: f32,        // offset 20: 透明度
    mass: f32,         // offset 24: 質量（ノイズ変形の強さに使用）
    _pad: f32,         // offset 28: パディング
}

@group(2) @binding(0) var<uniform> material: DreamBubbleMaterial;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    // UV中心を(0,0)、端を(-1〜1)に正規化
    let p = (uv - 0.5) * 2.0;

    // ---- ノイズ変形（角度に基づくsin波で円を歪ませる） ----
    let angle = atan2(p.y, p.x);
    // 質量が大きいほど変形が強くなる（mass=1.0でほぼ真円、mass=12.0で最大変形）
    let deform_strength = clamp(material.mass / 12.0, 0.0, 1.0) * 0.15;
    let deform = sin(angle * 3.0 + material.time * 1.8) * sin(angle * 5.0 - material.time * 1.2) * deform_strength;
    let radius = length(p);
    // 変形後の「見かけの半径」で描画判定
    let effective_radius = radius - deform;

    // 円外は完全透明
    let soft_edge = 0.05;
    if effective_radius > 1.0 + soft_edge {
        return vec4<f32>(0.0);
    }

    // ---- ソフトグロー（中心から外縁へ放射状減衰） ----
    let glow_falloff = pow(clamp(1.0 - effective_radius, 0.0, 1.0), 2.5);
    let base_fill = glow_falloff * 0.35;

    // ---- 虹色屈折（シャボン玉の表面） ----
    // 角度 + 時間で虹色をシフトする
    let iridescent_phase = angle + material.time * 0.6;
    let iris_r = 0.5 + 0.5 * sin(iridescent_phase * 1.0);
    let iris_g = 0.5 + 0.5 * sin(iridescent_phase * 1.0 + 2.094); // 120度ずらし
    let iris_b = 0.5 + 0.5 * sin(iridescent_phase * 1.0 + 4.189); // 240度ずらし
    // 輪郭付近（外縁）にのみ虹色を乗せる。中心付近はベース色を維持
    let rim_ratio = clamp((effective_radius - 0.5) / 0.5, 0.0, 1.0);
    let iridescent_color = vec3<f32>(iris_r, iris_g, iris_b) * rim_ratio * 0.4;

    // ---- リム発光（輪郭のリング状ハイライト） ----
    let rim_inner = 0.80;
    let rim_outer = 1.0;
    let rim_t = smoothstep(rim_outer, rim_inner, effective_radius)
              * smoothstep(rim_inner - 0.2, rim_inner, effective_radius);
    let rim_intensity = rim_t * 0.6;

    // ---- スペキュラハイライト（左上の固定輝点） ----
    let specular_center = vec2<f32>(-0.35, -0.40);
    let spec_dist = length(p - specular_center);
    let specular = pow(clamp(1.0 - spec_dist / 0.30, 0.0, 1.0), 3.0) * 0.5;

    // ---- 合成 ----
    let base_rgb = material.color.rgb;
    var final_rgb = base_rgb * base_fill
                  + iridescent_color
                  + base_rgb * rim_intensity
                  + vec3<f32>(1.0) * specular;

    // 外縁のソフトフェード
    let alpha_fade = smoothstep(1.0 + soft_edge, 0.85, effective_radius);
    let final_alpha = material.alpha * alpha_fade;

    return vec4<f32>(final_rgb, final_alpha);
}
