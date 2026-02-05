//! ソウルのイベントオブザーバー（ハンドラ）

use super::*;
use crate::constants::*;
use crate::events::{
    OnExhausted, OnSoulRecruited, OnStressBreakdown, OnTaskAssigned, OnTaskCompleted,
};
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::unassign_task;
use crate::world::map::WorldMap;
use rand::Rng;

pub fn on_task_assigned(on: On<OnTaskAssigned>, _q_souls: Query<&mut DamnedSoul>) {
    let soul_entity = on.entity;
    let event = on.event();
    info!(
        "OBSERVER: Soul {:?} assigned to task {:?} ({:?})",
        soul_entity, event.task_entity, event.work_type
    );
}

pub fn on_task_completed(on: On<OnTaskCompleted>, _q_souls: Query<&mut DamnedSoul>) {
    let soul_entity = on.entity;
    let event = on.event();
    info!(
        "OBSERVER: Soul {:?} completed task {:?} ({:?})",
        soul_entity, event.task_entity, event.work_type
    );
}

pub fn on_soul_recruited(on: On<OnSoulRecruited>, _q_souls: Query<&mut DamnedSoul>) {
    let soul_entity = on.entity;
    let event = on.event();
    info!(
        "OBSERVER: Soul {:?} recruited by Familiar {:?}",
        soul_entity, event.familiar_entity
    );
}

pub fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &mut Path,
        Option<&mut crate::systems::logistics::Inventory>,
        Option<&crate::relationships::CommandedBy>,
    )>,
    world_map: Res<WorldMap>,
    mut queries: crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
) {
    let soul_entity = on.entity;
    if let Ok((entity, transform, mut _soul, mut task, mut path, mut inventory_opt, under_command)) =
        q_souls.get_mut(soul_entity)
    {
        info!("OBSERVER: Soul {:?} had a stress breakdown!", entity);

        commands
            .entity(entity)
            .insert(StressBreakdown { is_frozen: true });

        if !matches!(*task, AssignedTask::None) {
            unassign_task(
                &mut commands,
                entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                inventory_opt.as_deref_mut(),
                None,
                &mut queries,
                &world_map,
                true,
            );
        }

        if under_command.is_some() {
            commands
                .entity(entity)
                .remove::<CommandedBy>();
        }
    }
}

pub fn on_exhausted(
    on: On<OnExhausted>,
    mut commands: Commands,
    q_spots: Query<&crate::systems::soul_ai::gathering::GatheringSpot>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut IdleState,
        &mut AssignedTask,
        &mut Path,
        &mut Destination,
        Option<&mut crate::systems::logistics::Inventory>,
        Option<&crate::relationships::CommandedBy>,
    )>,
    world_map: Res<WorldMap>,
    mut queries: crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
) {
    let soul_entity = on.entity;
    if let Ok((
        entity,
        transform,
        mut idle,
        mut task,
        mut path,
        mut dest,
        mut inventory_opt,
        under_command_opt,
    )) = q_souls.get_mut(soul_entity)
    {
        info!(
            "OBSERVER: Soul {:?} is exhausted, heading to gathering area",
            entity
        );

        if under_command_opt.is_some() {
            commands
                .entity(entity)
                .remove::<CommandedBy>();
        }

        if !matches!(*task, AssignedTask::None) {
            unassign_task(
                &mut commands,
                entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                inventory_opt.as_deref_mut(),
                None,
                &mut queries,
                &world_map,
                true,
            );
        }

        if idle.behavior != IdleBehavior::ExhaustedGathering {
            if idle.behavior != IdleBehavior::Gathering {
                let mut rng = rand::thread_rng();
                idle.gathering_behavior = match rng.gen_range(0..4) {
                    0 => GatheringBehavior::Wandering,
                    1 => GatheringBehavior::Sleeping,
                    2 => GatheringBehavior::Standing,
                    _ => GatheringBehavior::Dancing,
                };
                idle.gathering_behavior_timer = 0.0;
                idle.gathering_behavior_duration = rng.gen_range(60.0..90.0);
                idle.needs_separation = true;
            }
            idle.behavior = IdleBehavior::ExhaustedGathering;
            idle.idle_timer = 0.0;
            let mut rng = rand::thread_rng();
            idle.behavior_duration = rng.gen_range(2.0..4.0);
        }

        // 最寄りの集会所を探す
        let current_pos = transform.translation.truncate();
        let gathering_center = q_spots
            .iter()
            .min_by(|a, b| {
                a.center
                    .distance_squared(current_pos)
                    .partial_cmp(&b.center.distance_squared(current_pos))
                    .unwrap()
            })
            .map(|s| s.center);

        if let Some(center) = gathering_center {
            let dist_from_center = (center - current_pos).length();

            if dist_from_center > TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE {
                dest.0 = center;
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}
