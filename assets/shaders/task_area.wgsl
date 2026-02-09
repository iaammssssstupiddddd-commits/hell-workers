#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

struct TaskAreaMaterial {
    color: vec4<f32>,   // offset 0
    size: vec2<f32>,    // offset 16
    time: f32,          // offset 24
    state: u32,         // offset 28
}

@group(2) @binding(0) var<uniform> material: TaskAreaMaterial;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    
    // ピクセル座標に変換
    let pixel_pos = uv * material.size;
    let min_dist = min(pixel_pos.x, min(pixel_pos.y, min(material.size.x - pixel_pos.x, material.size.y - pixel_pos.y)));
    
    var final_color = vec4<f32>(0.0);
    
    // 四隅からの距離を計算（グラデーション用）
    let d_tl = length(pixel_pos - vec2(0.0, 0.0));
    let d_tr = length(pixel_pos - vec2(material.size.x, 0.0));
    let d_bl = length(pixel_pos - vec2(0.0, material.size.y));
    let d_br = length(pixel_pos - vec2(material.size.x, material.size.y));
    let corner_dist = min(min(d_tl, d_tr), min(d_bl, d_br));

    // 0. ベースの塗りつぶし (Fill)
    if (material.state > 0u) {
        // Selected/Editing 時の塗りを少し強める
        let base_alpha = select(0.12, 0.2, material.state >= 2u);
        final_color = material.color * base_alpha;
    } else {
        final_color = material.color * 0.04;
    }
    
    // 1. グラデーションの強化 (Vignette)
    // 半径をさらに広げ、中心に向かってより深く浸透させる
    let grad_radius = 160.0; 
    let grad_falloff = 2.2; // より急峻な減衰で端を強調
    if (material.state > 0u) {
        let normalized_dist = clamp(corner_dist / grad_radius, 0.0, 1.0);
        let grad_intensity = pow(1.0 - normalized_dist, grad_falloff) * 0.65;
        final_color += material.color * grad_intensity;
    }
    
    // 2. コーナーマーカー (L字)
    let marker_size = 14.0;
    let marker_thickness = 2.2;
    
    let is_marker_h = (pixel_pos.x < marker_size || pixel_pos.x > material.size.x - marker_size) && (min_dist < marker_thickness);
    let is_marker_v = (pixel_pos.y < marker_size || pixel_pos.y > material.size.y - marker_size) && (min_dist < marker_thickness);
    
    if (is_marker_h || is_marker_v) {
        let marker_intensity = select(0.7, 1.0, material.state >= 1u);
        final_color = material.color * marker_intensity;
    }
    
    // 3. 境界線 (細化: 1.0px)
    let border_thickness = 1.0;
    if (min_dist < border_thickness) {
        if (material.state == 1u) {
            // Hover: 点線
            let dash_size = 12.0;
            let gap_size = 6.0;
            let total_dash = dash_size + gap_size;
            let pos_along_border = pixel_pos.x + pixel_pos.y;
            if (f32(fract(pos_along_border / total_dash)) < (dash_size / total_dash)) {
                final_color = material.color * 0.9;
            }
        } else if (material.state >= 2u) {
            // Selected/Editing: 実線
            var pulse = 1.0;
            if (material.state == 3u) {
                pulse = 0.8 + 0.2 * sin(material.time * 12.0);
            }
            final_color = material.color * pulse;
        } else {
            // Idle
            final_color = material.color * 0.35;
        }
    }
    
    // アルファの最小値を保証（完全に消えないように）
    final_color.a = max(final_color.a, 0.04);
    
    return final_color;
}
