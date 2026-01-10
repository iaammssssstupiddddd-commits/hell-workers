//! 疲労（Fatigue）システム
//!
//! 魂の疲労値を管理し、タスク実行中に増加、待機中に減少させる。

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::UnderCommand;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

/// 疲労の増減を管理するシステム
/// - タスク実行中: 疲労増加 (+0.01/秒)
/// - 使役中の待機: 疲労減少（遅い） (-0.01/秒)
/// - 通常の待機: 疲労減少（速い） (-0.05/秒)
pub fn fatigue_update_system(
    time: Res<Time>,
    mut q_souls: Query<(&mut DamnedSoul, &AssignedTask, Option<&UnderCommand>)>,
) {
    let dt = time.delta_secs();

    for (mut soul, task, under_command) in q_souls.iter_mut() {
        let has_task = !matches!(*task, AssignedTask::None);

        if has_task {
            // タスク実行中: 疲労増加
            soul.fatigue = (soul.fatigue + dt * 0.01).min(1.0);
        } else if under_command.is_some() {
            // 使役中の待機: 疲労減少（遅い）
            soul.fatigue = (soul.fatigue - dt * 0.01).max(0.0);
        } else {
            // 通常の待機: 疲労減少（速い）
            soul.fatigue = (soul.fatigue - dt * 0.05).max(0.0);
        }
    }
}

/// 疲労が限界に達したら強制的に休憩させるシステム
/// 疲労が90%を超えるとやる気が急速に低下
pub fn fatigue_penalty_system(time: Res<Time>, mut q_souls: Query<&mut DamnedSoul>) {
    let dt = time.delta_secs();
    for mut soul in q_souls.iter_mut() {
        if soul.fatigue > 0.9 {
            soul.motivation = (soul.motivation - dt * 0.5).max(0.0);
        }
    }
}
