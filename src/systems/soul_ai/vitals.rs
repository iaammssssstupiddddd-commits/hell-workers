//! ワーカーのバイタル（疲労、ストレス、やる気）を管理するモジュール

use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::events::{OnExhausted, OnStressBreakdown};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{FamiliarSpatialGrid, SpatialGridOps};

use bevy::prelude::*;

// ============================================================
// 疲労（Fatigue）システム
// ============================================================

/// 疲労の増減を管理するシステム
pub fn fatigue_update_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &mut DamnedSoul,
        &AssignedTask,
        &IdleState,
        Option<&UnderCommand>,
    )>,
) {
    let dt = time.delta_secs();

    for (entity, mut soul, task, idle, under_command) in q_souls.iter_mut() {
        let has_task = !matches!(*task, AssignedTask::None);

        if has_task {
            // タスク実行中: 疲労増加
            soul.fatigue = (soul.fatigue + dt * FATIGUE_WORK_RATE).min(1.0);
        } else if under_command.is_some() {
            // 使役中の待機: 疲労減少（遅い）
            soul.fatigue = (soul.fatigue - dt * FATIGUE_RECOVERY_RATE_COMMANDED).max(0.0);
        } else {
            // 通常の待機: 疲労減少（速い）
            soul.fatigue = (soul.fatigue - dt * FATIGUE_RECOVERY_RATE_IDLE).max(0.0);
        }

        if soul.fatigue > FATIGUE_GATHERING_THRESHOLD
            && idle.behavior != IdleBehavior::ExhaustedGathering
        {
            commands.trigger(OnExhausted { entity });
        }
    }
}

/// 疲労が限界に達した際のペナルティシステム
pub fn fatigue_penalty_system(time: Res<Time>, mut q_souls: Query<&mut DamnedSoul>) {
    let dt = time.delta_secs();
    for mut soul in q_souls.iter_mut() {
        if soul.fatigue > FATIGUE_MOTIVATION_PENALTY_THRESHOLD {
            soul.motivation = (soul.motivation - dt * FATIGUE_MOTIVATION_PENALTY_RATE).max(0.0);
        }
    }
}

// ============================================================
// ストレス（Stress）システム
// ============================================================

/// ストレスの更新とブレイクダウン状態管理システム
pub fn stress_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_souls: Query<(
        Entity,
        &mut DamnedSoul,
        &AssignedTask,
        &IdleState,
        Option<&UnderCommand>,
        Option<&mut StressBreakdown>,
    )>,
) {
    let dt = time.delta_secs();

    for (entity, mut soul, task, idle, under_command, breakdown_opt) in q_souls.iter_mut() {
        let has_task = !matches!(*task, AssignedTask::None);
        let is_gathering = matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        );

        if has_task {
            soul.stress = (soul.stress + dt * STRESS_WORK_RATE).min(1.0);
        } else if is_gathering {
            soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_GATHERING).max(0.0);
        } else if under_command.is_some() {
            // 待機中（使役下）= 変化なし
        } else {
            soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_IDLE).max(0.0);
        }

        if soul.stress >= 1.0 {
            if breakdown_opt.is_none() {
                commands.trigger(OnStressBreakdown { entity });
            }
        } else if let Some(mut breakdown) = breakdown_opt {
            if soul.stress <= STRESS_RECOVERY_THRESHOLD {
                commands.entity(entity).remove::<StressBreakdown>();
            } else if soul.stress <= STRESS_FREEZE_RECOVERY_THRESHOLD && breakdown.is_frozen {
                breakdown.is_frozen = false;
            }
        }
    }
}

/// 監視による追加ストレスの更新システム
pub fn supervision_stress_system(
    time: Res<Time>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: Query<(&Transform, &mut DamnedSoul, &AssignedTask)>,
) {
    let dt = time.delta_secs();

    for (soul_transform, mut soul, task) in q_souls.iter_mut() {
        if matches!(*task, AssignedTask::None) {
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
                        SUPERVISION_IDLE_MULTIPLIER
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

        if best_influence > 0.0 {
            let supervision_stress = best_influence * dt * SUPERVISION_STRESS_SCALE;
            soul.stress = (soul.stress + supervision_stress).min(1.0);
        }
    }
}

// ============================================================
// やる気（Motivation）システム
// ============================================================

/// やる気・怠惰の更新システム
pub fn motivation_system(
    time: Res<Time>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: Query<(
        &Transform,
        &mut DamnedSoul,
        &AssignedTask,
        Option<&UnderCommand>,
    )>,
) {
    let dt = time.delta_secs();

    for (soul_transform, mut soul, task, under_command) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        let has_task = !matches!(*task, AssignedTask::None);

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
                        SUPERVISION_IDLE_MULTIPLIER
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

        if best_influence > 0.0 {
            soul.motivation =
                (soul.motivation + best_influence * dt * SUPERVISION_MOTIVATION_SCALE).min(1.0);
            soul.laziness =
                (soul.laziness - best_influence * dt * SUPERVISION_LAZINESS_SCALE).max(0.0);
        } else if has_task || under_command.is_some() {
            soul.motivation = (soul.motivation - dt * MOTIVATION_LOSS_RATE_ACTIVE).max(0.0);
            soul.laziness = (soul.laziness - dt * LAZINESS_LOSS_RATE_ACTIVE).max(0.0);
        } else {
            soul.motivation = (soul.motivation - dt * MOTIVATION_LOSS_RATE_IDLE).max(0.0);
            soul.laziness = (soul.laziness + dt * LAZINESS_GAIN_RATE_IDLE).min(1.0);
        }
    }
}

/// ホバー線の描画用コンポーネント
#[derive(Component)]
pub struct HoverLineIndicator;

/// 使い魔にホバーした際、使役中の魂との間に線を引く
pub fn familiar_hover_visualization_system(
    mut commands: Commands,
    hovered_entity: Res<crate::interface::selection::HoveredEntity>,
    q_familiars: Query<(&GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<(&GlobalTransform, &UnderCommand), With<DamnedSoul>>,
    q_lines: Query<Entity, With<HoverLineIndicator>>,
    mut gizmos: Gizmos,
) {
    for entity in q_lines.iter() {
        commands.entity(entity).despawn();
    }

    if let Some(hovered) = hovered_entity.0 {
        if let Ok((fam_transform, _)) = q_familiars.get(hovered) {
            let fam_pos = fam_transform.translation().truncate();

            for (soul_transform, under_command) in q_souls.iter() {
                if under_command.0 == hovered {
                    let soul_pos = soul_transform.translation().truncate();
                    gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.7));
                }
            }
        }
    }
}

/// タスク完了時のモチベーションボーナス
pub fn on_task_completed_motivation_bonus(
    trigger: On<crate::events::OnTaskCompleted>,
    mut q_souls: Query<&mut DamnedSoul>,
) {
    let event = trigger.event();
    if let Ok(mut soul) = q_souls.get_mut(event.entity) {
        let bonus = match event.work_type {
            crate::systems::jobs::WorkType::Chop | crate::systems::jobs::WorkType::Mine => {
                MOTIVATION_BONUS_GATHER
            }
            crate::systems::jobs::WorkType::Haul => MOTIVATION_BONUS_HAUL,
            crate::systems::jobs::WorkType::Build => MOTIVATION_BONUS_BUILD,
        };

        if bonus > 0.0 {
            soul.motivation = (soul.motivation + bonus).min(1.0);
        }
    }
}

/// 激励イベントによる効果適用
pub fn on_encouraged_effect(
    trigger: On<crate::events::OnEncouraged>,
    mut q_souls: Query<&mut DamnedSoul>,
) {
    let event = trigger.event();
    if let Ok(mut soul) = q_souls.get_mut(event.soul_entity) {
        soul.motivation = (soul.motivation + ENCOURAGEMENT_MOTIVATION_BONUS).min(1.0);
        soul.stress = (soul.stress + ENCOURAGEMENT_STRESS_PENALTY).min(1.0);
    }
}

/// リクルート時のバイタル変化
pub fn on_soul_recruited_effect(
    trigger: On<crate::events::OnSoulRecruited>,
    mut q_souls: Query<&mut DamnedSoul>,
) {
    let event = trigger.event();
    if let Ok(mut soul) = q_souls.get_mut(event.entity) {
        soul.motivation = (soul.motivation + RECRUIT_MOTIVATION_BONUS).min(1.0);
        soul.stress = (soul.stress + RECRUIT_STRESS_PENALTY).min(1.0);
    }
}
