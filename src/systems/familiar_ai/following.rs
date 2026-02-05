use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::Familiar;
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

/// 部下が使い魔を追尾するシステム
pub fn following_familiar_system(
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &AssignedTask,
            &CommandedBy,
            &IdleState,
            &mut Destination,
            &mut Path,
        ),
        (With<DamnedSoul>, Without<Familiar>),
    >,
    q_familiars: Query<(&Transform, &Familiar), With<Familiar>>,
) {
    for (_soul_entity, soul_transform, task, commanded_by, idle, mut dest, mut path) in
        q_souls.iter_mut()
    {
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            continue;
        }
        if !matches!(task, AssignedTask::None) {
            continue;
        }

        if let Ok((fam_transform, familiar)) = q_familiars.get(commanded_by.0) {
            let fam_pos = fam_transform.translation.truncate();
            let soul_pos = soul_transform.translation.truncate();
            let command_radius = familiar.command_radius;

            // 使い魔の影響範囲内に移動する
            let distance_sq = soul_pos.distance_squared(fam_pos);
            let radius_sq = command_radius * command_radius;
            
            // 影響範囲外にいる場合は、使い魔の位置に向かって移動する
            if distance_sq > radius_sq {
                // 既存の目的地と新しい目的地の距離をチェック
                // 2.0以上離れている場合のみ更新（pathfinding_systemのChanged<Destination>を確実に発火させるため）
                if dest.0.distance_squared(fam_pos) > 4.0 {
                    dest.0 = fam_pos;
                    // パスを完全にリセット（pathfinding_systemで再計算される）
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            } else {
                // 影響範囲内にいる場合は、既存のパスをクリアして停止
                // ただし、既にパスが完了している場合は何もしない
                if !path.waypoints.is_empty() && path.current_index < path.waypoints.len() {
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
        }
    }
}
