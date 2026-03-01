// Dream泡パーティクル用フラグメントシェーダー（UI空間 / UiMaterial用）
// 星雲（Nebula）内包 + 有機的変形 + 睡眠の呼吸 + 全体明度の向上 + バブルクラスター

#import bevy_ui::ui_vertex_output::UiVertexOutput
#import bevy_render::view::View

@group(0) @binding(0) var<uniform> view: View;

struct DreamBubbleUiMaterial {
    color: vec4<f32>,       // offset 0:  ベース色 (16 bytes)
    time: f32,              // offset 16: 経過時間
    alpha: f32,             // offset 20: 透明度
    mass: f32,              // offset 24: 質量
    velocity_dir: vec2<f32>,// offset 28: 移動方向（正規化済み）
}

@group(1) @binding(0) var<uniform> material: DreamBubbleUiMaterial;

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

// 画面中央フェード定数
const CENTER_FADE_START: f32 = 0.4;  
const CENTER_FADE_END: f32 = 1.0;    
const CENTER_MIN_ALPHA: f32 = 0.4;  

// 1つの円に対するグロー・リム・スペキュラを計算して返す
fn bubble_at(p: vec2<f32>, center: vec2<f32>, r: f32, time: f32) -> vec4<f32> {
    let local = p - center;
    let angle = atan2(local.y, local.x);
    let radius = length(local) / r; 

    let soft_edge = 0.08;
    if radius > 1.0 + soft_edge {
        return vec4<f32>(0.0);
    }

    let t = time * 0.8;
    let breath = 0.85 + 0.15 * sin(time * 1.5);

    // 星雲テクスチャ（Nebula）
    let nebula_noise = fbm(local * 2.0 / r + vec2<f32>(t * 0.8, t * -0.5));
    let glow_falloff = pow(clamp(1.0 - radius, 0.0, 1.0), 1.2); 
    
    // ソフトグローと星雲の合成色
    let fill_intensity = (glow_falloff * 0.4 + nebula_noise * 0.9) * breath;
    let base_rgb = material.color.rgb;
    let bright_rgb = mix(base_rgb, vec3<f32>(1.5), pow(nebula_noise, 1.5) * 0.9);
    let base_fill = bright_rgb * fill_intensity;

    // 虹色屈折
    let iridescent_phase = angle + time * 0.6;
    let iris_r = 0.5 + 0.5 * sin(iridescent_phase);
    let iris_g = 0.5 + 0.5 * sin(iridescent_phase + 2.094);
    let iris_b = 0.5 + 0.5 * sin(iridescent_phase + 4.189);
    let rim_ratio = clamp((radius - 0.4) / 0.6, 0.0, 1.0);
    let iridescent = vec3<f32>(iris_r, iris_g, iris_b) * rim_ratio * 0.6 * breath;

    // リム発光
    let rim_t = smoothstep(1.0, 0.80, radius) * smoothstep(0.50, 0.80, radius);
    let rim = rim_t * (0.8 + 0.4 * fbm(local * 5.0 / r + t)) * breath;

    // スペキュラ（左上に固定）
    let spec_center = center + vec2<f32>(-0.30, -0.35) * r;
    let spec_dist = length(p - spec_center);
    let specular = pow(clamp(1.0 - spec_dist / (0.35 * r), 0.0, 1.0), 2.5) * 0.8;

    // 境界の霧散（Foggy Edges）
    let edge_noise = fbm(local * 8.0 / r - t);
    let edge_fade_dist = 1.0 + soft_edge * edge_noise;
    let alpha_fade = smoothstep(edge_fade_dist, 0.80, radius);

    let rgb = base_fill + iridescent + base_rgb * rim + vec3<f32>(1.0) * specular;
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), alpha_fade);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let p = (uv - 0.5) * 2.0;

    let bubble_count = select(select(1u, 2u, material.mass >= 3.0), 3u, material.mass >= 6.0);
    
    let t = material.time * 0.8;
    var final_color = vec4<f32>(0.0);

    if bubble_count == 1u {
        // ---- 1泡: FBMノイズ変形を適用した通常の泡 ----
        let deform_val = fbm(p * 2.5 - vec2<f32>(t, t * 1.5)) * 2.0 - 1.0;
        let deform_strength = 0.15 + clamp(material.mass / 12.0, 0.0, 1.0) * 0.10;
        let deform = deform_val * deform_strength;
        let single_effective_radius = length(p) - deform;

        let soft_edge = 0.08;
        if single_effective_radius <= 1.0 + soft_edge {
            let breath = 0.85 + 0.15 * sin(material.time * 1.5);

            let nebula_noise = fbm(p * 2.0 + vec2<f32>(t * 0.8, t * -0.5));
            let glow_falloff = pow(clamp(1.0 - single_effective_radius, 0.0, 1.0), 1.2); 
            
            let fill_intensity = (glow_falloff * 0.4 + nebula_noise * 0.9) * breath;
            let base_rgb = material.color.rgb;
            let bright_rgb = mix(base_rgb, vec3<f32>(1.5), pow(nebula_noise, 1.5) * 0.9);
            let base_fill = bright_rgb * fill_intensity;

            let angle_single = atan2(p.y, p.x);
            let iridescent_phase = angle_single + material.time * 0.6;
            let iris_r = 0.5 + 0.5 * sin(iridescent_phase);
            let iris_g = 0.5 + 0.5 * sin(iridescent_phase + 2.094);
            let iris_b = 0.5 + 0.5 * sin(iridescent_phase + 4.189);
            let rim_ratio = clamp((single_effective_radius - 0.4) / 0.6, 0.0, 1.0);
            let iridescent = vec3<f32>(iris_r, iris_g, iris_b) * rim_ratio * 0.6 * breath;

            let rim_t = smoothstep(1.0, 0.80, single_effective_radius)
                      * smoothstep(0.50, 0.80, single_effective_radius);
            let rim = rim_t * (0.8 + 0.4 * fbm(p * 5.0 + t)) * breath;

            let spec_center = vec2<f32>(-0.32, -0.38);
            let spec_dist = length(p - spec_center);
            let specular = pow(clamp(1.0 - spec_dist / 0.35, 0.0, 1.0), 2.5) * 0.8;

            let edge_noise = fbm(p * 8.0 - t);
            let edge_fade_dist = 1.0 + soft_edge * edge_noise;
            let alpha_fade = smoothstep(edge_fade_dist, 0.80, single_effective_radius);

            let rgb_sum = base_fill + iridescent + base_rgb * rim + vec3<f32>(1.0) * specular;
            final_color = vec4<f32>(
                clamp(rgb_sum, vec3<f32>(0.0), vec3<f32>(1.0)),
                alpha_fade
            );
        }

    } else if bubble_count == 2u {
        // ---- 2泡: 横並び（移動方向に沿って配置） ----
        let rot_angle = material.time * 0.3;
        let cos_r = cos(rot_angle);
        let sin_r = sin(rot_angle);
        let offset = 0.45;
        let c1 = vec2<f32>(-offset * cos_r, -offset * sin_r);
        let c2 = vec2<f32>( offset * cos_r,  offset * sin_r);
        let r_sub = 0.60;

        let b1 = bubble_at(p, c1, r_sub, material.time);
        let b2 = bubble_at(p, c2, r_sub, material.time);

        final_color = vec4<f32>(max(b1.rgb, b2.rgb), max(b1.a, b2.a));

    } else {
        // ---- 3泡: 三角形配置 ----
        let rot_angle = material.time * 0.2;
        let cos_r = cos(rot_angle);
        let sin_r = sin(rot_angle);
        let offset = 0.42;
        let c1 = vec2<f32>(offset * cos_r, offset * sin_r);
        let angle2 = rot_angle + 2.094; 
        let c2 = vec2<f32>(offset * cos(angle2), offset * sin(angle2));
        let angle3 = rot_angle + 4.189; 
        let c3 = vec2<f32>(offset * cos(angle3), offset * sin(angle3));
        let r_sub = 0.55;

        let b1 = bubble_at(p, c1, r_sub, material.time);
        let b2 = bubble_at(p, c2, r_sub, material.time);
        let b3 = bubble_at(p, c3, r_sub, material.time);

        final_color = vec4<f32>(
            max(max(b1.rgb, b2.rgb), b3.rgb),
            max(max(b1.a, b2.a), b3.a)
        );
    }

    // ---- 画面中央フェード ----
    let screen_uv = (in.position.xy - view.viewport.xy) / view.viewport.zw; // 0..1
    let screen_center_offset = abs(screen_uv - 0.5) * 2.0; // 0 (中央) .. 1 (端)
    let screen_dist = length(screen_center_offset);
    let center_t = clamp((screen_dist - CENTER_FADE_START) / (CENTER_FADE_END - CENTER_FADE_START), 0.0, 1.0);
    let center_fade = CENTER_MIN_ALPHA + center_t * (1.0 - CENTER_MIN_ALPHA);

    let current_alpha = clamp(material.alpha * 1.3, 0.0, 1.0);
    let final_alpha = final_color.a * current_alpha * center_fade;
    // 発光（加算）的な見え方を抑え、確実に透明に近づくようにRGBもcenter_fadeで暗くする
    let output_rgb = final_color.rgb * center_fade;
    return vec4<f32>(output_rgb, final_alpha);
}
