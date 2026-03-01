// Dream泡パーティクル用フラグメントシェーダー（World空間 / Mesh2d用）
// 星雲（Nebula）内包 + 有機的変形 + 睡眠の呼吸 + 全体明度の向上

#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
}

// -- 疑似乱数とノイズ関数 --
fn hash(p: vec2<f32>) -> f32 {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(q.x) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash(i + vec2<f32>(0.0, 0.0));
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pp = p;
    let rot = mat2x2<f32>(0.866, 0.5, -0.5, 0.866);
    for (var i = 0; i < 4; i = i + 1) {
        v += a * noise(pp);
        pp = rot * pp * 2.0 + vec2<f32>(100.0, 100.0);
        a *= 0.5;
    }
    return v;
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

    // ---- 有機的な変形（Morphing） ----
    let t = material.time * 0.8; // 時間変化を少し早める
    let deform_val = fbm(p * 2.5 - vec2<f32>(t, t * 1.5)) * 2.0 - 1.0;
    // 質量が小さくても明らかにうごめくよう、ベース変形強度を 0.15 に引き上げ
    let deform_strength = 0.15 + clamp(material.mass / 12.0, 0.0, 1.0) * 0.10;
    let deform = deform_val * deform_strength;
    
    let radius = length(p);
    // 変形後の「見かけの半径」で描画判定
    let effective_radius = radius - deform;

    // 円外は完全透明
    let soft_edge = 0.08;
    if effective_radius > 1.0 + soft_edge {
        return vec4<f32>(0.0);
    }

    // ---- 睡眠の呼吸（Breathing） ----
    let breath = 0.85 + 0.15 * sin(material.time * 1.5);

    // ---- 星雲テクスチャ（Nebula）とベースカラーの強調 ----
    // ノイズの模様を大きくし、コントラストを強める
    let nebula_noise = fbm(p * 2.0 + vec2<f32>(t * 0.8, t * -0.5));
    let glow_falloff = pow(clamp(1.0 - effective_radius, 0.0, 1.0), 1.2); 
    
    // 星雲の明るい部分をより白く際立たせ、内部の雲のような質感を強調
    let fill_intensity = (glow_falloff * 0.4 + nebula_noise * 0.9) * breath;
    let base_rgb = material.color.rgb;
    let bright_rgb = mix(base_rgb, vec3<f32>(1.5), pow(nebula_noise, 1.5) * 0.9);
    let base_fill = bright_rgb * fill_intensity;

    // ---- 虹色屈折（シャボン玉の表面） ----
    let angle = atan2(p.y, p.x);
    let iridescent_phase = angle + material.time * 0.6;
    let iris_r = 0.5 + 0.5 * sin(iridescent_phase * 1.0);
    let iris_g = 0.5 + 0.5 * sin(iridescent_phase * 1.0 + 2.094);
    let iris_b = 0.5 + 0.5 * sin(iridescent_phase * 1.0 + 4.189);
    // 輪郭付近にのみ配置
    let rim_ratio = clamp((effective_radius - 0.4) / 0.6, 0.0, 1.0);
    let iridescent_color = vec3<f32>(iris_r, iris_g, iris_b) * rim_ratio * 0.6 * breath;

    // リム発光（輪郭のリング状ハイライト）
    let rim_inner = 0.80;
    let rim_outer = 1.0;
    let rim_t = smoothstep(rim_outer, rim_inner, effective_radius)
              * smoothstep(rim_inner - 0.3, rim_inner, effective_radius);
    // リムも細かく揺らぐようにノイズを合成（強度アップ）
    let rim_intensity = rim_t * (0.8 + 0.4 * fbm(p * 5.0 + t)) * breath;

    // ---- スペキュラハイライト（左上の固定輝点） ----
    let specular_center = vec2<f32>(-0.35, -0.40);
    let spec_dist = length(p - specular_center);
    let specular = pow(clamp(1.0 - spec_dist / 0.35, 0.0, 1.0), 2.5) * 0.8; 

    // ---- 合成 ----
    var final_rgb = base_fill
                  + iridescent_color
                  + base_rgb * rim_intensity
                  + vec3<f32>(1.0) * specular;

    // 境界に霧散（Foggy Edges）を適用
    let edge_noise = fbm(p * 8.0 - t);
    let edge_fade_dist = 1.0 + soft_edge * edge_noise; // ノイズによる境界線の揺らぎ
    let alpha_fade = smoothstep(edge_fade_dist, 0.80, effective_radius);
    
    // 全体明度の向上に合わせてアルファも底上げ
    let current_alpha = clamp(material.alpha * 1.3, 0.0, 1.0); 
    let final_alpha = current_alpha * alpha_fade;

    return vec4<f32>(final_rgb, final_alpha);
}
