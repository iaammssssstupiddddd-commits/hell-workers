use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{ActiveCommand, Familiar, UnderCommand};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::vitals::helpers;
use crate::systems::spatial::{FamiliarSpatialGrid, SpatialGridOps};

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

        let best_influence =
            helpers::calculate_best_influence(soul_pos, &nearby_familiar_entities, &q_familiars);

        if best_influence > 0.0 {
            let supervision_stress = best_influence * dt * SUPERVISION_STRESS_SCALE;
            soul.stress = (soul.stress + supervision_stress).min(1.0);
        }
    }
}

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

        let best_influence =
            helpers::calculate_best_influence(soul_pos, &nearby_familiar_entities, &q_familiars);

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
