use bevy::prelude::*;

use hw_core::assigned_task::AssignedTask;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};

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

            let distance_sq = soul_pos.distance_squared(fam_pos);
            let radius_sq = command_radius * command_radius;

            if distance_sq > radius_sq {
                if dest.0.distance_squared(fam_pos) > 4.0 {
                    dest.0 = fam_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            } else if !path.waypoints.is_empty() && path.current_index < path.waypoints.len() {
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}
