use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{DreamQuality, GatheringBehavior, IdleBehavior};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot};
use crate::systems::soul_ai::helpers::query_types::IdleVisualSoulQuery;
use crate::systems::spatial::{GatheringSpotSpatialGrid, SpatialGridOps};

/// 怠惰行動のビジュアルフィードバック
pub fn idle_visual_system(
    q_spots: Query<&GatheringSpot>,
    spot_grid: Res<GatheringSpotSpatialGrid>,
    mut query: IdleVisualSoulQuery,
) {
    // idle_behavior_system で定義されている定数を使用
    const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;

    for (mut transform, mut sprite, idle, soul, task, participating_in, dream) in query.iter_mut() {
        if !matches!(task, AssignedTask::None) {
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
            sprite.color = Color::WHITE;
            continue;
        }

        match idle.behavior {
            IdleBehavior::Sleeping => {
                transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                sprite.color = match dream.quality {
                    DreamQuality::VividDream => Color::srgba(0.5, 0.6, 0.9, 1.0),
                    DreamQuality::NightTerror => Color::srgba(0.8, 0.4, 0.4, 1.0),
                    _ => Color::srgba(0.6, 0.6, 0.7, 1.0),
                };
            }
            IdleBehavior::Sitting => {
                transform.rotation = Quat::IDENTITY;
                transform.scale.y = 0.8;
                sprite.color = Color::srgba(0.8, 0.8, 0.8, 1.0);
            }
            IdleBehavior::Wandering => {
                transform.rotation = Quat::IDENTITY;
                sprite.color = Color::WHITE;
            }
            IdleBehavior::Escaping => {
                // 逃走中: 少し傾けて走っている感じ + 青白い色（パニック）
                transform.rotation = Quat::from_rotation_z(-0.1);
                // 色を少し青白く
                sprite.color = Color::srgba(0.8, 0.9, 1.0, 1.0);
                // 軽く点滅（パニック感）
                let panic_pulse = (idle.total_idle_time * 8.0).sin() * 0.05 + 0.95;
                transform.scale = Vec3::splat(panic_pulse);
            }
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                let gathering_center = if let Some(p) = participating_in {
                    q_spots.get(p.0).ok().map(|s| s.center)
                } else {
                    let pos = transform.translation.truncate();
                    let nearby = spot_grid.get_nearby_in_radius(pos, GATHERING_LEAVE_RADIUS * 2.0);
                    nearby
                        .iter()
                        .filter_map(|&e| q_spots.get(e).ok())
                        .min_by(|a, b| {
                            a.center
                                .distance_squared(pos)
                                .partial_cmp(&b.center.distance_squared(pos))
                                .unwrap()
                        })
                        .map(|s| s.center)
                };

                if let Some(center) = gathering_center {
                    let current_pos = transform.translation.truncate();
                    let dist_from_center = (center - current_pos).length();
                    let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

                    if !has_arrived {
                        transform.rotation = Quat::IDENTITY;
                        transform.scale = Vec3::ONE;

                        if idle.behavior == IdleBehavior::ExhaustedGathering {
                            sprite.color = Color::srgba(0.7, 0.6, 0.8, 0.9);
                        } else {
                            sprite.color = Color::srgba(0.85, 0.75, 1.0, 0.85);
                        }
                    } else {
                        sprite.color = Color::srgba(0.8, 0.7, 1.0, 0.7);

                        match idle.gathering_behavior {
                            GatheringBehavior::Wandering => {
                                transform.rotation = Quat::IDENTITY;
                                // パルスアニメーションで動いている感じを出す
                                let pulse_speed = 1.5;
                                let scale_offset =
                                    (idle.total_idle_time * pulse_speed).sin() * 0.05 + 1.0;
                                transform.scale = Vec3::splat(scale_offset);
                                // 少し明るい色にしてうろつき中であることを示す
                                sprite.color = Color::srgba(0.9, 0.8, 1.0, 0.85);
                            }
                            GatheringBehavior::Sleeping => {
                                transform.rotation =
                                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                                sprite.color = match dream.quality {
                                    DreamQuality::VividDream => Color::srgba(0.5, 0.5, 0.9, 0.7),
                                    DreamQuality::NightTerror => Color::srgba(0.8, 0.4, 0.5, 0.6),
                                    _ => Color::srgba(0.6, 0.5, 0.8, 0.6),
                                };
                                let breath = (idle.total_idle_time * 0.3).sin() * 0.02 + 0.95;
                                transform.scale = Vec3::splat(breath);
                            }
                            GatheringBehavior::Standing => {
                                transform.rotation = Quat::IDENTITY;
                                // 呼吸アニメーションを少し大きく（3%）
                                let breath = (idle.total_idle_time * 0.4).sin() * 0.03 + 1.0;
                                transform.scale = Vec3::splat(breath);
                            }
                            GatheringBehavior::Dancing => {
                                // 揺れを大きく（±0.3ラジアン ≈ ±17度）、速く（5倍速）
                                let sway_angle = (idle.total_idle_time * 5.0).sin() * 0.3;
                                transform.rotation = Quat::from_rotation_z(sway_angle);
                                // バウンスを大きく（15%）、速く（6倍速）
                                let bounce = (idle.total_idle_time * 6.0).sin() * 0.15 + 1.0;
                                transform.scale = Vec3::new(1.0, bounce, 1.0);
                                // 明るいピンク色で目立たせる
                                sprite.color = Color::srgba(1.0, 0.7, 1.0, 1.0);
                            }
                        }
                    }
                } else {
                    transform.rotation = Quat::IDENTITY;
                    sprite.color = Color::WHITE;
                }
            }
        }

        // やる気が高い場合は視覚効果を上書きするが、集会中とタスク中は除外
        if soul.motivation > 0.5
            && !matches!(
                idle.behavior,
                IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
            )
            && matches!(task, AssignedTask::None)
        {
            sprite.color = Color::srgb(1.0, 1.0, 0.8);
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
        }
    }
}
