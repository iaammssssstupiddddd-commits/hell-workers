use super::components::{DreamGainUiParticle, DreamIconAbsorb, DreamTrailGhost};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::interface::ui::components::{UiNodeRegistry, UiRoot, UiSlot};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

/// 「漂い→吸引」2相イージング
fn bubble_ease(t: f32) -> f32 {
    if t < 0.7 {
        let p = t / 0.7;
        p * p * 0.2
    } else {
        let p = (t - 0.7) / 0.3;
        0.2 + p * p * 0.8
    }
}

/// 4次ベジェ曲線の位置計算
fn bezier4(t: f32, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> Vec2 {
    let u = 1.0 - t;
    u.powi(4) * p0
        + 4.0 * u.powi(3) * t * p1
        + 6.0 * u.powi(2) * t.powi(2) * p2
        + 4.0 * u * t.powi(3) * p3
        + t.powi(4) * p4
}

pub fn ui_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    ui_nodes: Res<UiNodeRegistry>,
    q_ui_root: Query<Entity, With<UiRoot>>,
    mut q_icon: Query<&mut DreamIconAbsorb>,
    mut q_particles: Query<(
        Entity,
        &mut DreamGainUiParticle,
        &mut Node,
        &mut BackgroundColor,
    )>,
) {
    let dt = time.delta_secs();
    let ui_root = q_ui_root.iter().next();

    // 合体ターゲットの位置を事前収集（借用衝突を回避）
    let target_positions: Vec<(Entity, Vec2)> = q_particles
        .iter()
        .map(|(e, _, n, _)| {
            let pos = Vec2::new(
                match n.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
                match n.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
            );
            (e, pos)
        })
        .collect();

    for (entity, mut particle, mut node, mut color) in q_particles.iter_mut() {
        particle.lifetime -= dt;
        if particle.lifetime <= 0.0 {
            // 到着時: アイコンにパルスを通知
            if particle.merging_into.is_none() {
                if let Some(icon_entity) =
                    ui_nodes.get_slot(UiSlot::DreamPoolIcon)
                {
                    if let Ok(mut absorb) = q_icon.get_mut(icon_entity) {
                        absorb.pulse_count += 1;
                    }
                }
            }
            commands.entity(entity).try_despawn();
            continue;
        }

        // 吸収中パーティクル（merge target に吸い寄せられている）
        if let Some(target) = particle.merging_into {
            particle.merge_timer -= dt;
            let progress = 1.0 - (particle.merge_timer / DREAM_UI_MERGE_DURATION).clamp(0.0, 1.0);

            // 事前収集した位置から target の現在位置を取得
            if let Some(&(_, target_pos)) = target_positions.iter().find(|(e, _)| *e == target) {
                let current_left = match node.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                };
                let current_top = match node.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                };
                let current = Vec2::new(current_left, current_top);
                let new_pos = current.lerp(target_pos, progress);
                node.left = Val::Px(new_pos.x);
                node.top = Val::Px(new_pos.y);

                // サイズ縮小
                let base =
                    DREAM_UI_PARTICLE_SIZE + particle.merge_count as f32 * DREAM_UI_MERGE_SIZE_BONUS;
                let size = base * (1.0 - progress);
                node.width = Val::Px(size);
                node.height = Val::Px(size);

                // アルファフェード
                color.0 = color.0.with_alpha(0.9 * (1.0 - progress));
            }

            if particle.merge_timer <= 0.0 {
                commands.entity(entity).try_despawn();
            }
            continue;
        }

        // --- 通常移動パーティクル ---
        let t = 1.0 - (particle.lifetime / particle.max_lifetime).clamp(0.0, 1.0);
        let mapped_t = bubble_ease(t);

        // Bezier位置
        let bezier_pos = bezier4(
            mapped_t,
            particle.start_pos,
            particle.control_point_1,
            particle.control_point_2,
            particle.control_point_3,
            particle.target_pos,
        );

        // 位置揺らぎ（漂いフェーズで強く、吸引で減衰）
        let drift_strength =
            (1.0 - ((t - 0.5) / 0.3).clamp(0.0, 1.0)) * DREAM_UI_BUBBLE_DRIFT_STRENGTH;
        let drift_x = (particle.phase * 2.0).sin() * drift_strength;
        let drift_y = (particle.phase * 2.7 + 0.8).cos() * drift_strength;
        let final_pos = bezier_pos + Vec2::new(drift_x, drift_y);

        // 前フレーム位置を記録（trail用）
        particle.prev_pos = Vec2::new(
            match node.left {
                Val::Px(v) => v,
                _ => final_pos.x,
            },
            match node.top {
                Val::Px(v) => v,
                _ => final_pos.y,
            },
        );

        node.left = Val::Px(final_pos.x);
        node.top = Val::Px(final_pos.y);

        // サイズ計算
        let base =
            DREAM_UI_PARTICLE_SIZE + particle.merge_count as f32 * DREAM_UI_MERGE_SIZE_BONUS;
        let current_size = base * (1.0 - mapped_t * 0.7);

        // Wobble（形状揺れ） - 漂い中強く、吸引で減衰
        particle.phase += dt * 6.0;
        let wobble_strength = 1.0 - ((t - 0.7) / 0.3).clamp(0.0, 1.0);
        let wx = (particle.phase * 3.5).sin() * 1.5 * wobble_strength;
        let wy = (particle.phase * 4.8 + 1.7).cos() * 1.5 * wobble_strength;
        node.width = Val::Px(current_size + wx);
        node.height = Val::Px(current_size + wy);

        // 色変化: mapped_t > 0.6 でシアン→白方向
        let base_color = if mapped_t > 0.6 {
            let white_t = ((mapped_t - 0.6) / 0.4).clamp(0.0, 1.0);
            let r = 0.65 + white_t * 0.35;
            let g = 0.9 + white_t * 0.1;
            let b = 1.0;
            Color::srgb(r, g, b)
        } else {
            Color::srgb(0.65, 0.9, 1.0)
        };

        // フェード: 序盤0.1秒でフェードイン、以降alpha=0.9固定
        let alpha = if t < 0.067 {
            // 0.1s / 1.5s ≈ 0.067
            (t / 0.067).clamp(0.0, 1.0) * 0.9
        } else {
            0.9
        };
        color.0 = base_color.with_alpha(alpha);

        // Trail生成（漂いフェーズ中心: t 0.1~0.75）
        particle.trail_cooldown -= dt;
        if particle.trail_cooldown <= 0.0 && t > 0.1 && t < 0.75 {
            particle.trail_cooldown = DREAM_UI_TRAIL_INTERVAL;
            let trail_size = current_size * DREAM_UI_TRAIL_SIZE_RATIO;
            if let Some(root) = ui_root {
                let trail = commands
                    .spawn((
                        DreamTrailGhost {
                            lifetime: DREAM_UI_TRAIL_LIFETIME,
                            max_lifetime: DREAM_UI_TRAIL_LIFETIME,
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(final_pos.x),
                            top: Val::Px(final_pos.y),
                            width: Val::Px(trail_size),
                            height: Val::Px(trail_size),
                            ..default()
                        },
                        ImageNode::new(assets.dream_bubble.clone()),
                        BackgroundColor(
                            Color::srgb(0.65, 0.9, 1.0).with_alpha(DREAM_UI_TRAIL_ALPHA),
                        ),
                        ZIndex(-2),
                        Name::new("DreamTrailGhost"),
                    ))
                    .id();
                commands.entity(root).add_child(trail);
            }
        }
    }
}

/// 合体判定システム
pub fn ui_particle_merge_system(
    mut q_particles: Query<(Entity, &mut DreamGainUiParticle, &Node)>,
) {
    // 全パーティクルの位置を収集
    let positions: Vec<(Entity, Vec2, f32, bool)> = q_particles
        .iter()
        .map(|(e, p, n)| {
            let pos = Vec2::new(
                match n.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
                match n.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
            );
            let t = 1.0 - (p.lifetime / p.max_lifetime).clamp(0.0, 1.0);
            let merging = p.merging_into.is_some();
            (e, pos, t, merging)
        })
        .collect();

    // ペアワイズ距離チェック（1フレーム1ペアのみ）
    let mut merge_pair: Option<(Entity, Entity)> = None;
    'outer: for i in 0..positions.len() {
        if positions[i].3 {
            continue; // 既に吸収中
        }
        if positions[i].2 < 0.15 {
            continue; // 初期フェーズ
        }
        for j in (i + 1)..positions.len() {
            if positions[j].3 {
                continue;
            }
            if positions[j].2 < 0.15 {
                continue;
            }
            let dist = positions[i].1.distance(positions[j].1);
            if dist < DREAM_UI_MERGE_RADIUS {
                // tが小さい方（後方）を吸収対象に
                if positions[i].2 < positions[j].2 {
                    merge_pair = Some((positions[i].0, positions[j].0)); // i→jに吸収される
                } else {
                    merge_pair = Some((positions[j].0, positions[i].0)); // j→iに吸収される
                }
                break 'outer;
            }
        }
    }

    if let Some((absorbed, absorber)) = merge_pair {
        // 吸収者の merge_count を確認
        if let Ok((_, absorber_p, _)) = q_particles.get(absorber) {
            if absorber_p.merge_count >= DREAM_UI_MERGE_MAX_COUNT {
                return;
            }
        }

        // absorbed: merging_into設定
        if let Ok((_, mut absorbed_p, _)) = q_particles.get_mut(absorbed) {
            absorbed_p.merging_into = Some(absorber);
            absorbed_p.merge_timer = DREAM_UI_MERGE_DURATION;
        }

        // absorber: merge_count加算
        if let Ok((_, mut absorber_p, _)) = q_particles.get_mut(absorber) {
            absorber_p.merge_count = (absorber_p.merge_count + 1).min(DREAM_UI_MERGE_MAX_COUNT);
        }
    }
}

/// Trail ゴーストのフェードアウトシステム
pub fn dream_trail_ghost_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_ghosts: Query<(Entity, &mut DreamTrailGhost, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut ghost, mut color) in q_ghosts.iter_mut() {
        ghost.lifetime -= dt;
        if ghost.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let alpha = (ghost.lifetime / ghost.max_lifetime) * DREAM_UI_TRAIL_ALPHA;
        color.0 = color.0.with_alpha(alpha);
    }
}

/// DreamPoolIcon の吸収パルスシステム
pub fn dream_icon_absorb_system(
    time: Res<Time>,
    theme: Res<UiTheme>,
    mut q_icon: Query<(&mut Node, &mut BackgroundColor, &mut DreamIconAbsorb)>,
) {
    let dt = time.delta_secs();
    for (mut node, mut color, mut absorb) in q_icon.iter_mut() {
        if absorb.pulse_count > 0 {
            absorb.timer = DREAM_ICON_ABSORB_DURATION;
            absorb.pulse_count = 0;
        }

        if absorb.timer > 0.0 {
            absorb.timer -= dt;
            let progress = 1.0 - (absorb.timer / DREAM_ICON_ABSORB_DURATION).clamp(0.0, 1.0);
            let sin_val = (progress * std::f32::consts::PI).sin();

            // サイズパルス: 16→20→16
            let size = DREAM_ICON_BASE_SIZE
                + (DREAM_ICON_PULSE_SIZE - DREAM_ICON_BASE_SIZE) * sin_val;
            node.width = Val::Px(size);
            node.height = Val::Px(size);

            // 白フラッシュ
            let base = theme.colors.accent_soul_bright;
            let r = base.to_srgba().red + (1.0 - base.to_srgba().red) * sin_val * 0.5;
            let g = base.to_srgba().green + (1.0 - base.to_srgba().green) * sin_val * 0.5;
            let b = base.to_srgba().blue + (1.0 - base.to_srgba().blue) * sin_val * 0.5;
            color.0 = Color::srgb(r, g, b);
        } else {
            // 復帰
            node.width = Val::Px(DREAM_ICON_BASE_SIZE);
            node.height = Val::Px(DREAM_ICON_BASE_SIZE);
            color.0 = theme.colors.accent_soul_bright;
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

    let t_x = target_pos.x;
    let t_y = target_pos.y;
    let s_x = start_pos.x;
    let s_y = start_pos.y;

    let dist_up = s_y;
    let dist_down = viewport_size.y - s_y;
    let dist_left = s_x;
    let dist_right = viewport_size.x - s_x;

    let mut min_dist = dist_up;
    let mut pattern = 0;

    if dist_down < min_dist {
        min_dist = dist_down;
        pattern = 3;
    }
    if dist_left < min_dist {
        min_dist = dist_left;
        pattern = 2;
    }
    if dist_right < min_dist {
        pattern = 1;
    }

    let _ = min_dist; // suppress unused warning

    let (c1, c2, c3) =
        calculate_control_points(pattern, s_x, s_y, t_x, t_y, viewport_size, &mut rng);

    let phase: f32 = rand::random::<f32>() * std::f32::consts::TAU;

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
                phase,
                merge_count: 0,
                merging_into: None,
                merge_timer: 0.0,
                trail_cooldown: DREAM_UI_TRAIL_INTERVAL,
                prev_pos: start_pos,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(start_pos.x),
                top: Val::Px(start_pos.y),
                width: Val::Px(DREAM_UI_PARTICLE_SIZE),
                height: Val::Px(DREAM_UI_PARTICLE_SIZE),
                ..default()
            },
            ImageNode::new(assets.dream_bubble.clone()),
            BackgroundColor(Color::srgb(0.65, 0.9, 1.0).with_alpha(0.0)), // フェードインで開始
            ZIndex(0),
            Name::new("DreamGainUiParticle"),
        ))
        .id();

    commands.entity(ui_root).add_child(particle);
}

// 制御点の計算ロジック（4次ベジェ曲線用）
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
            let c1_x = s_x - 30.0;
            let c1_y = rng.gen_range(0.0..30.0);
            let c2_x = s_x + (t_x - s_x) * 0.3;
            let c2_y = 10.0;
            let c3_x = t_x - 30.0;
            let c3_y = 10.0;
            (
                Vec2::new(c1_x, c1_y),
                Vec2::new(c2_x, c2_y),
                Vec2::new(c3_x, c3_y),
            )
        }
        1 => {
            let c1_x = viewport_size.x - rng.gen_range(10.0..30.0);
            let c1_y = s_y - 20.0;
            let c2_x = viewport_size.x - 10.0;
            let c2_y = s_y + (t_y - s_y) * 0.4;
            let c3_x = viewport_size.x - 10.0;
            let c3_y = t_y + 30.0;
            (
                Vec2::new(c1_x, c1_y),
                Vec2::new(c2_x, c2_y),
                Vec2::new(c3_x, c3_y),
            )
        }
        2 => {
            let c1_x = 20.0;
            let c1_y = s_y - 40.0;
            let c2_x = 20.0;
            let c2_y = 20.0;
            let c3_x = s_x + (t_x - s_x) * 0.6;
            let c3_y = 20.0;
            (
                Vec2::new(c1_x, c1_y),
                Vec2::new(c2_x, c2_y),
                Vec2::new(c3_x, c3_y),
            )
        }
        _ => {
            let c1_x = s_x + 40.0;
            let c1_y = viewport_size.y - 20.0;
            let c2_x = viewport_size.x - 20.0;
            let c2_y = viewport_size.y - 20.0;
            let c3_x = viewport_size.x - 20.0;
            let c3_y = s_y + (t_y - s_y) * 0.7;
            (
                Vec2::new(c1_x, c1_y),
                Vec2::new(c2_x, c2_y),
                Vec2::new(c3_x, c3_y),
            )
        }
    }
}
