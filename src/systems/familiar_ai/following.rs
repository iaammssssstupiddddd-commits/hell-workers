use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::{Familiar, UnderCommand};
use crate::systems::soul_ai::execution::AssignedTask;
use bevy::prelude::*;

/// 部下が使い魔を追尾するシステム
pub fn following_familiar_system(
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &AssignedTask,
            &UnderCommand,
            &IdleState,
            &mut Destination,
            &mut Path,
        ),
        (With<DamnedSoul>, Without<Familiar>),
    >,
    q_familiars: Query<&Transform, With<Familiar>>,
) {
    for (_soul_entity, soul_transform, task, under_command, idle, mut dest, mut path) in
        q_souls.iter_mut()
    {
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            continue;
        }
        if !matches!(task, AssignedTask::None) {
            continue;
        }

        if let Ok(fam_transform) = q_familiars.get(under_command.0) {
            let fam_pos = fam_transform.translation.truncate();
            let soul_pos = soul_transform.translation.truncate();

            // 使い魔の近くに集まる
            if soul_pos.distance_squared(fam_pos) > (64.0f32).powi(2) {
                dest.0 = fam_pos;
                // パスは次のpathfinding_systemで更新されるため、ここではクリアのみ
                path.waypoints.clear();
            }
        }
    }
}
