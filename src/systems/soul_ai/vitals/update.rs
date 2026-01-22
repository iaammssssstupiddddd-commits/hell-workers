use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use crate::entities::familiar::UnderCommand;
use crate::events::{OnExhausted, OnStressBreakdown};
use crate::systems::soul_ai::task_execution::AssignedTask;

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
