// Dream泡パーティクル用フラグメントシェーダー（UI空間 / UiMaterial用）
// ソフトグロー + 虹色屈折 + スペキュラハイライト + リム発光 + ノイズ変形 + バブルクラスター

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

// 画面中央フェード定数
const CENTER_FADE_START: f32 = 0.4;  // フェード開始距離（正規化）
const CENTER_FADE_END: f32 = 1.0;    // フェード終了距離
const CENTER_MIN_ALPHA: f32 = 0.4;  // 中央での最小alpha係数

// 1つの円に対するグロー・リム・スペキュラを計算して返す
fn bubble_at(p: vec2<f32>, center: vec2<f32>, r: f32, time: f32) -> vec4<f32> {
    let local = p - center;
    let angle = atan2(local.y, local.x);
    let radius = length(local) / r; // 正規化半径（1.0が円周）

    if radius > 1.1 {
        return vec4<f32>(0.0);
    }

    // ソフトグロー
    let glow = pow(clamp(1.0 - radius, 0.0, 1.0), 2.5) * 0.5;

    // 虹色屈折（外縁付近のみ）
    let iridescent_phase = angle + time * 0.6;
    let iris_r = 0.5 + 0.5 * sin(iridescent_phase);
    let iris_g = 0.5 + 0.5 * sin(iridescent_phase + 2.094);
    let iris_b = 0.5 + 0.5 * sin(iridescent_phase + 4.189);
    let rim_ratio = clamp((radius - 0.5) / 0.5, 0.0, 1.0);
    let iridescent = vec3<f32>(iris_r, iris_g, iris_b) * rim_ratio * 0.5;

    // リム発光
    let rim_t = smoothstep(1.0, 0.80, radius) * smoothstep(0.60, 0.80, radius);
    let rim = rim_t * 0.7;

    // スペキュラ（左上に固定）
    let spec_center = center + vec2<f32>(-0.30, -0.35) * r;
    let spec_dist = length(p - spec_center);
    let specular = pow(clamp(1.0 - spec_dist / (0.25 * r), 0.0, 1.0), 3.0) * 0.6;

    // アルファ（外縁フェード）
    let alpha_fade = smoothstep(1.1, 0.85, radius);

    let rgb = material.color.rgb * (glow + rim) + iridescent + vec3<f32>(1.0) * specular;
    return vec4<f32>(rgb, alpha_fade);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // UV中心を(0,0)、端を(-1〜1)に正規化
    let p = (uv - 0.5) * 2.0;

    // 質量からサブ泡の数を決定
    // mass < 3.0: 1泡, mass < 6.0: 2泡, それ以上: 3泡
    let bubble_count = select(select(1u, 2u, material.mass >= 3.0), 3u, material.mass >= 6.0);

    // ---- ノイズ変形（質量に応じたいびつな形 / 1泡の場合のみ適用） ----
    let angle_single = atan2(p.y, p.x);
    let deform_strength = clamp(material.mass / 12.0, 0.0, 1.0) * 0.18;
    let deform = sin(angle_single * 3.0 + material.time * 1.8)
               * sin(angle_single * 5.0 - material.time * 1.2)
               * deform_strength;
    let single_effective_radius = length(p) - deform;

    var final_color = vec4<f32>(0.0);

    if bubble_count == 1u {
        // ---- 1泡: ノイズ変形を適用した通常の泡 ----
        let soft_edge = 0.05;
        if single_effective_radius <= 1.0 + soft_edge {
            let glow = pow(clamp(1.0 - single_effective_radius, 0.0, 1.0), 2.5) * 0.5;

            let iridescent_phase = angle_single + material.time * 0.6;
            let iris_r = 0.5 + 0.5 * sin(iridescent_phase);
            let iris_g = 0.5 + 0.5 * sin(iridescent_phase + 2.094);
            let iris_b = 0.5 + 0.5 * sin(iridescent_phase + 4.189);
            let rim_ratio = clamp((single_effective_radius - 0.5) / 0.5, 0.0, 1.0);
            let iridescent = vec3<f32>(iris_r, iris_g, iris_b) * rim_ratio * 0.5;

            let rim_t = smoothstep(1.0, 0.80, single_effective_radius)
                      * smoothstep(0.60, 0.80, single_effective_radius);
            let rim = rim_t * 0.7;

            let spec_center = vec2<f32>(-0.32, -0.38);
            let spec_dist = length(p - spec_center);
            let specular = pow(clamp(1.0 - spec_dist / 0.28, 0.0, 1.0), 3.0) * 0.6;

            let alpha_fade = smoothstep(1.0 + soft_edge, 0.85, single_effective_radius);
            final_color = vec4<f32>(
                material.color.rgb * (glow + rim) + iridescent + vec3<f32>(1.0) * specular,
                alpha_fade
            );
        }

    } else if bubble_count == 2u {
        // ---- 2泡: 横並び（移動方向に沿って配置） ----
        // ゆっくり回転してうごめく
        let rot_angle = material.time * 0.3;
        let cos_r = cos(rot_angle);
        let sin_r = sin(rot_angle);
        // 各泡は0.42の半径、中心から0.45ずれた位置
        let offset = 0.45;
        let c1 = vec2<f32>(-offset * cos_r, -offset * sin_r);
        let c2 = vec2<f32>( offset * cos_r,  offset * sin_r);
        let r_sub = 0.60;

        let b1 = bubble_at(p, c1, r_sub, material.time);
        let b2 = bubble_at(p, c2, r_sub, material.time);

        // 2泡を最大値合成（明るい方を残す）
        final_color = vec4<f32>(max(b1.rgb, b2.rgb), max(b1.a, b2.a));

    } else {
        // ---- 3泡: 三角形配置 ----
        let rot_angle = material.time * 0.2;
        let cos_r = cos(rot_angle);
        let sin_r = sin(rot_angle);
        let offset = 0.42;
        // 三角形の頂点（120度間隔）
        let c1 = vec2<f32>(
            offset * cos_r,
            offset * sin_r
        );
        let angle2 = rot_angle + 2.094; // +120度
        let c2 = vec2<f32>(
            offset * cos(angle2),
            offset * sin(angle2)
        );
        let angle3 = rot_angle + 4.189; // +240度
        let c3 = vec2<f32>(
            offset * cos(angle3),
            offset * sin(angle3)
        );
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
    // in.position はフラグメントシェーダーでは frag coord (ピクセル座標)
    // view.viewport は (x_origin, y_origin, width, height)
    let screen_uv = (in.position.xy - view.viewport.xy) / view.viewport.zw; // 0..1
    let screen_center_offset = abs(screen_uv - 0.5) * 2.0; // 0 (中央) .. 1 (端)
    let screen_dist = length(screen_center_offset);
    let center_t = clamp((screen_dist - CENTER_FADE_START) / (CENTER_FADE_END - CENTER_FADE_START), 0.0, 1.0);
    let center_fade = CENTER_MIN_ALPHA + center_t * (1.0 - CENTER_MIN_ALPHA);

    return vec4<f32>(final_color.rgb, final_color.a * material.alpha * center_fade);
}
