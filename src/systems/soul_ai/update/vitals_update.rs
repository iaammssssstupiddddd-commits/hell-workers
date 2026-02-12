use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::events::OnExhausted;
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;

/// 疲労の増減を管理するシステム
pub fn fatigue_update_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &mut DamnedSoul,
        &AssignedTask,
        &IdleState,
        Option<&CommandedBy>,
    )>,
) {
    let dt = time.delta_secs();

    for (entity, soul, task, idle, under_command) in q_souls.iter_mut() {
        let (entity, mut soul, task, idle, under_command): (
            Entity,
            Mut<DamnedSoul>,
            &AssignedTask,
            &IdleState,
            Option<&CommandedBy>,
        ) = (entity, soul, task, idle, under_command);
        let has_task = !matches!(task, AssignedTask::None);
        let prev_fatigue = soul.fatigue;

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

        let crossed_exhausted_threshold =
            prev_fatigue <= FATIGUE_GATHERING_THRESHOLD && soul.fatigue > FATIGUE_GATHERING_THRESHOLD;

        if crossed_exhausted_threshold && idle.behavior != IdleBehavior::ExhaustedGathering {
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
