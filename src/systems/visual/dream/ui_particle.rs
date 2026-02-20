use super::components::DreamGainUiParticle;
use crate::assets::GameAssets;
use bevy::prelude::*;

pub fn ui_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_particles: Query<(Entity, &mut DreamGainUiParticle, &mut Node, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();

    for (entity, mut particle, mut node, mut color) in q_particles.iter_mut() {
        particle.lifetime -= dt;
        if particle.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        // ベジェ曲線による補間 (start -> control -> target)
        let t = 1.0 - (particle.lifetime / particle.max_lifetime).clamp(0.0, 1.0);
        // 軌跡が急カーブするため、時間経過は通常の Ease-out（2乗）で十分
        let t = 1.0 - (1.0 - t).powi(2);

        let p0 = particle.start_pos;
        let p1 = particle.control_point_1;
        let p2 = particle.control_point_2;
        let p3 = particle.control_point_3;
        let p4 = particle.target_pos;

        // 4次ベジェ曲線の計算: B(t) = (1-t)^4 p0 + 4(1-t)^3 t p1 + 6(1-t)^2 t^2 p2 + 4(1-t) t^3 p3 + t^4 p4
        let current_pos = (1.0 - t).powi(4) * p0 
            + 4.0 * (1.0 - t).powi(3) * t * p1 
            + 6.0 * (1.0 - t).powi(2) * t.powi(2) * p2 
            + 4.0 * (1.0 - t) * t.powi(3) * p3 
            + t.powi(4) * p4;

        node.left = Val::Px(current_pos.x);
        node.top = Val::Px(current_pos.y);

        // 半分過ぎたらフェードアウト開始
        if particle.lifetime < particle.max_lifetime * 0.5 {
            let alpha = (particle.lifetime / (particle.max_lifetime * 0.5)).clamp(0.0, 1.0);
            color.0 = color.0.with_alpha(alpha * 0.9);
        }
    }
}

// パーティクル生成用のユーティリティ関数
pub fn spawn_ui_particle(
    commands: &mut Commands,
    start_pos: Vec2,
    target_pos: Vec2,
    viewport_size: Vec2,
    lifetime: f32,
    ui_root: Entity,
    assets: &GameAssets,
) {
    let mut rng = rand::thread_rng();

    // 目標は右上（上辺と右辺）なので、Y方向へ先に上がるか、X方向に先に右へ行く軌道をランダムで描く
    let t_x = target_pos.x;
    let t_y = target_pos.y;
    let s_x = start_pos.x;
    let s_y = start_pos.y;

    // 画面中央を完全に回避するため、発生位置の「外側」の座標を制御点として強く設定する
    // 目的地は基本的に右上 (t_xは大きく、t_yは小さい)
    
    // Y=0が画面上端、X=0が画面左端。
    // 発生位置から最も近い画面端を算出し、その端へ逃げるパターンを選択する
    let dist_up = s_y;
    let dist_down = viewport_size.y - s_y;
    let dist_left = s_x;
    let dist_right = viewport_size.x - s_x;

    let mut min_dist = dist_up;
    let mut pattern = 0; // デフォルトは上端(0)へ逃げる

    if dist_down < min_dist {
        min_dist = dist_down;
        pattern = 3; // 下端(3)へ逃げる
    }
    if dist_left < min_dist {
        min_dist = dist_left;
        pattern = 2; // 左端(2)へ逃げる
    }
    if dist_right < min_dist {
        pattern = 1; // 右端(1)へ逃げる
    }

    let (c1, c2, c3) = calculate_control_points(
        pattern, s_x, s_y, t_x, t_y, viewport_size, &mut rng
    );

    let particle = commands
        .spawn((
            DreamGainUiParticle {
                lifetime,
                max_lifetime: lifetime,
                start_pos,
                target_pos,
                control_point_1: c1,
                control_point_2: c2,
                control_point_3: c3,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(start_pos.x),
                top: Val::Px(start_pos.y),
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                ..default()
            },
            ImageNode::new(assets.glow_circle.clone()),
            BackgroundColor(Color::srgb(0.65, 0.9, 1.0).with_alpha(0.9)),
            ZIndex(-1), // UI(Root)ノードの内部で背面に表示し、他のUI要素より後ろに潜らせる
            Name::new("DreamGainUiParticle"),
        ))
        .id();

    commands.entity(ui_root).add_child(particle);
}

// 制御点の計算ロジック（4次ベジェ曲線用）を下請け関数として分離
fn calculate_control_points(
    pattern: i32,
    s_x: f32,
    s_y: f32,
    t_x: f32,
    t_y: f32,
    viewport_size: Vec2,
    rng: &mut impl rand::Rng,
) -> (Vec2, Vec2, Vec2) {
    match pattern {
        0 => {
            // パターン0: 上辺を這って目的地へ
            // c1: 斜め左上へ逃げる
            let c1_x = s_x - 30.0;
            let c1_y = rng.gen_range(0.0..30.0); 
            // c2: 上辺（Yは0寄り）に張り付き、少し右へ
            let c2_x = s_x + (t_x - s_x) * 0.3;
            let c2_y = 10.0;
            // c3: 上辺を這い続け、目的地のX座標付近まで大きく右へ
            let c3_x = t_x - 30.0;
            let c3_y = 10.0;
            (Vec2::new(c1_x, c1_y), Vec2::new(c2_x, c2_y), Vec2::new(c3_x, c3_y))
        },
        1 => {
            // パターン1: 右辺を這って目的地へ
            // c1: 右上へ逃げる
            let c1_x = viewport_size.x - rng.gen_range(10.0..30.0);
            let c1_y = s_y - 20.0;
            // c2: 右辺に張り付き、少し上へ
            let c2_x = viewport_size.x - 10.0;
            let c2_y = s_y + (t_y - s_y) * 0.4;
            // c3: 右辺を這い続け、目的地のY座標付近まで大きく上へ
            let c3_x = viewport_size.x - 10.0;
            let c3_y = t_y + 30.0;
            (Vec2::new(c1_x, c1_y), Vec2::new(c2_x, c2_y), Vec2::new(c3_x, c3_y))
        },
        2 => {
            // パターン2: 左辺寄りから上辺を回る
            // c1: 左へ逃げる
            let c1_x = 20.0;
            let c1_y = s_y - 40.0;
            // c2: 左上（コーナー）へ
            let c2_x = 20.0;
            let c2_y = 20.0;
            // c3: 上辺に沿って右（目的地）へ
            let c3_x = s_x + (t_x - s_x) * 0.6;
            let c3_y = 20.0;
            (Vec2::new(c1_x, c1_y), Vec2::new(c2_x, c2_y), Vec2::new(c3_x, c3_y))
        },
        _ => {
            // パターン3: 下寄りから右辺を回る
            // c1: 下へ逃げる
            let c1_x = s_x + 40.0;
            let c1_y = viewport_size.y - 20.0;
            // c2: 右下（コーナー）へ
            let c2_x = viewport_size.x - 20.0;
            let c2_y = viewport_size.y - 20.0;
            // c3: 右辺に沿って上（目的地）へ
            let c3_x = viewport_size.x - 20.0;
            let c3_y = s_y + (t_y - s_y) * 0.7;
            (Vec2::new(c1_x, c1_y), Vec2::new(c2_x, c2_y), Vec2::new(c3_x, c3_y))
        }
    }
}
