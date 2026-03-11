use bevy::prelude::*;
use hw_core::soul::IdleBehavior;
use hw_jobs::AssignedTask;

use crate::familiar_ai::decide::task_management::FamiliarSoulQuery;

pub(super) fn collect_idle_members(
    squad: &[Entity],
    fatigue_threshold: f32,
    q_souls: &mut FamiliarSoulQuery,
) -> Vec<(Entity, Vec2)> {
    let mut idle_members = Vec::new();

    for &member_entity in squad {
        if let Ok(soul_data) = q_souls.get(member_entity) {
            let (_, transform, soul, task, _, _, idle, _, _, _) = soul_data;
            if matches!(*task, AssignedTask::None)
                && idle.behavior != IdleBehavior::ExhaustedGathering
                && soul.fatigue <= fatigue_threshold
            {
                idle_members.push((member_entity, transform.translation.truncate()));
            }
        }
    }

    idle_members
}
