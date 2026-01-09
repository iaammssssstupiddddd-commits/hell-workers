use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::interface::camera::MainCamera;
use crate::systems::work::{AssignedTask, FamiliarSpatialGrid};
use bevy::prelude::*;

/// やる気・怠惰・ストレスの更新システム
/// ストレスはタスク実行中に増加し、待機・集会中に減少する
pub fn motivation_system(
    mut commands: Commands,
    time: Res<Time>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &IdleState,
        Option<&UnderCommand>,
        Option<&mut StressBreakdown>,
    )>,
) {
    let dt = time.delta_secs();

    for (entity, soul_transform, mut soul, mut task, idle, under_command, breakdown_opt) in
        q_souls.iter_mut()
    {
        let soul_pos = soul_transform.translation.truncate();
        let has_task = !matches!(*task, AssignedTask::None);
        let is_gathering = matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        );

        // 空間グリッドを使用して近傍の使い魔のみをチェック
        let max_radius = TILE_SIZE * 10.0;
        let nearby_familiar_entities = familiar_grid.get_nearby_in_radius(soul_pos, max_radius);

        let best_influence = nearby_familiar_entities
            .iter()
            .filter_map(|&fam_entity| {
                let Ok((fam_transform, familiar, command)) = q_familiars.get(fam_entity) else {
                    return None;
                };
                let influence_center = fam_transform.translation.truncate();
                let distance_sq = soul_pos.distance_squared(influence_center);
                let radius_sq = familiar.command_radius * familiar.command_radius;

                if distance_sq < radius_sq {
                    let distance = distance_sq.sqrt();
                    let command_multiplier = if matches!(command.command, FamiliarCommand::Idle) {
                        0.4
                    } else {
                        1.0
                    };
                    let distance_factor = 1.0 - (distance / familiar.command_radius);
                    Some(familiar.efficiency * distance_factor * command_multiplier)
                } else {
                    None
                }
            })
            .fold(0.0_f32, |acc, x| acc.max(x));

        // --- モチベーションと怠惰の更新 ---
        if best_influence > 0.0 {
            soul.motivation = (soul.motivation + best_influence * dt * 4.0).min(1.0);
            soul.laziness = (soul.laziness - best_influence * dt * 2.5).max(0.0);
        } else if has_task || under_command.is_some() {
            soul.motivation = (soul.motivation - dt * 0.02).max(0.0);
            soul.laziness = (soul.laziness - dt * 0.1).max(0.0);
            soul.fatigue = (soul.fatigue + dt * 0.01).min(1.0);
        } else {
            soul.motivation = (soul.motivation - dt * 0.1).max(0.0);
            soul.laziness = (soul.laziness + dt * 0.05).min(1.0);
            soul.fatigue = (soul.fatigue - dt * 0.05).max(0.0);
        }

        // --- ストレスの更新 ---
        // タスク1つ約10-15秒、1-2タスクで100%に達するよう調整
        // 約10秒で100% → 0.105/秒
        if has_task {
            // タスク実行中
            if best_influence > 0.0 {
                // 監視されながら働く = 高ストレス（約10秒で100%）
                soul.stress = (soul.stress + best_influence * dt * 0.105).min(1.0);
            } else {
                // 監視なしで働く = 軽いストレス
                soul.stress = (soul.stress + dt * 0.03).min(1.0);
            }
        } else if is_gathering {
            // 集会中 = 最速回復（約25秒で0%）
            soul.stress = (soul.stress - dt * 0.04).max(0.0);
        } else if under_command.is_some() || best_influence > 0.0 {
            // 待機中（範囲内）= 変化なし
            // 何もしない
        } else {
            // 待機中（範囲外）= リラックス（約50秒で0%）
            soul.stress = (soul.stress - dt * 0.02).max(0.0);
        }

        // --- ブレイクダウン状態管理 ---
        if soul.stress >= 1.0 {
            // ストレス限界 → ブレイクダウン発動
            if breakdown_opt.is_none() {
                commands
                    .entity(entity)
                    .insert(StressBreakdown { is_frozen: true });
                // タスクを放棄
                if has_task {
                    *task = AssignedTask::None;
                    info!("STRESS: Soul {:?} abandoned task due to breakdown", entity);
                }
                // 使役を解除
                if under_command.is_some() {
                    commands.entity(entity).remove::<UnderCommand>();
                    info!(
                        "STRESS: Soul {:?} entered breakdown, released from command",
                        entity
                    );
                }
            }
        } else if let Some(mut breakdown) = breakdown_opt {
            if soul.stress <= 0.7 {
                // 完全回復 → ブレイクダウン解除
                commands.entity(entity).remove::<StressBreakdown>();
            } else if soul.stress <= 0.9 && breakdown.is_frozen {
                // 動けるようになる（使役はまだ拒否）
                breakdown.is_frozen = false;
            }
        }
    }
}

/// 疲労が限界に達したら強制的に休憩させるシステム
pub fn fatigue_system(time: Res<Time>, mut q_souls: Query<&mut DamnedSoul>) {
    let dt = time.delta_secs();
    for mut soul in q_souls.iter_mut() {
        // 疲労が限界に達したらやる気が徐々に下がる（毎フレーム0.5ではなく時間ベース）
        if soul.fatigue > 0.9 {
            soul.motivation = (soul.motivation - dt * 0.5).max(0.0);
        }
    }
}

/// 使い魔にホバーした際、使役中の魂との間に細い線を引く
pub fn familiar_hover_visualization_system(
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_familiars: Query<(Entity, &GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<(&GlobalTransform, &crate::entities::familiar::UnderCommand), With<DamnedSoul>>,
    mut gizmos: Gizmos,
) {
    let Ok(window) = q_window.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = q_camera.get_single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            for (fam_entity, fam_transform, _) in q_familiars.iter() {
                let fam_pos = fam_transform.translation().truncate();

                // マウスが使い魔の上にあるかチェック
                if fam_pos.distance(world_pos) < TILE_SIZE * 0.5 {
                    // 使役中の魂全員（UnderCommand(fam_entity)を持つソウル）に対して線を引く
                    for (soul_transform, under_command) in q_souls.iter() {
                        if under_command.0 == fam_entity {
                            let soul_pos = soul_transform.translation().truncate();
                            gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.4));
                        }
                    }
                }
            }
        }
    }
}
