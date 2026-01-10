use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use crate::entities::familiar::UnderCommand;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

/// ストレスの更新とブレイクダウン状態管理システム
/// ストレスはタスク実行中に増加し、待機・集会中に減少する
pub fn stress_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &IdleState,
        &mut crate::systems::logistics::Inventory,
        Option<&UnderCommand>,
        Option<&mut StressBreakdown>,
    )>,
) {
    let dt = time.delta_secs();

    for (
        entity,
        transform,
        mut soul,
        mut task,
        idle,
        mut inventory,
        under_command,
        breakdown_opt,
    ) in q_souls.iter_mut()
    {
        let soul_pos = transform.translation.truncate();
        let has_task = !matches!(*task, AssignedTask::None);
        let is_gathering = matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        );

        // --- ストレスの更新 ---
        // タスク1つ約10-15秒、1-2タスクで100%に達するよう調整
        // 約10秒で100% → 0.105/秒
        if has_task {
            // タスク実行中 = 監視なしで働く = 軽いストレス
            soul.stress = (soul.stress + dt * 0.03).min(1.0);
        } else if is_gathering {
            // 集会中 = 最速回復（約25秒で0%）
            soul.stress = (soul.stress - dt * 0.04).max(0.0);
        } else if under_command.is_some() {
            // 待機中（使役下）= 変化なし
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
                    // アイテムをドロップ
                    if let Some(item_entity) = inventory.0.take() {
                        commands.entity(item_entity).insert((
                            Visibility::Visible,
                            Transform::from_xyz(soul_pos.x, soul_pos.y, 0.6),
                        ));
                        commands
                            .entity(item_entity)
                            .remove::<crate::systems::logistics::ClaimedBy>();
                    }

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

/// 監視ストレスの更新システム
/// 使い魔の監視範囲内でタスクを実行している魂に追加ストレスを与える
pub fn supervision_stress_system(
    time: Res<Time>,
    familiar_grid: Res<crate::systems::work::FamiliarSpatialGrid>,
    q_familiars: Query<(
        &Transform,
        &crate::entities::familiar::Familiar,
        &crate::entities::familiar::ActiveCommand,
    )>,
    mut q_souls: Query<(&Transform, &mut DamnedSoul, &AssignedTask)>,
) {
    use crate::constants::TILE_SIZE;
    use crate::entities::familiar::FamiliarCommand;

    let dt = time.delta_secs();

    for (soul_transform, mut soul, task) in q_souls.iter_mut() {
        let has_task = !matches!(*task, AssignedTask::None);
        if !has_task {
            continue;
        }

        let soul_pos = soul_transform.translation.truncate();
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

        // 監視されながら働く = 追加の高ストレス
        if best_influence > 0.0 {
            // 基本ストレス(0.03)に加えて監視ストレスを追加（合計で約0.105/秒になるよう）
            let supervision_stress = best_influence * dt * 0.075;
            soul.stress = (soul.stress + supervision_stress).min(1.0);
        }
    }
}
