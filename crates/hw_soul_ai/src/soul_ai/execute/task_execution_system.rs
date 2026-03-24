//! メインのタスク実行システム。
//!
//! Soul の AssignedTask ステートマシンを毎フレーム駆動する。

use bevy::prelude::*;
use hw_core::events::OnTaskCompleted;
use hw_core::relationships::WorkingOn;
use hw_core::visual::SoulTaskHandles;
use hw_logistics::Wheelbarrow;
use hw_world::WorldMapRead;
use hw_world::pathfinding::PathfindingContext;

use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskQueries};
use crate::soul_ai::execute::task_execution::handler::dispatch::run_task_handler;
use crate::soul_ai::execute::task_execution::types::AssignedTask;
use crate::soul_ai::helpers::query_types::TaskExecutionSoulQuery;
use crate::soul_ai::helpers::work::unassign_task;

#[allow(clippy::too_many_arguments)]
pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: TaskExecutionSoulQuery,
    mut queries: TaskQueries,
    soul_handles: Res<SoulTaskHandles>,
    time: Res<Time>,
    world_map: WorldMapRead,
    mut pf_context: Local<PathfindingContext>,
    q_wheelbarrows: Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
    q_entities: Query<Entity>,
) {
    for (
        soul_entity,
        soul_transform,
        mut soul,
        mut task,
        mut dest,
        mut path,
        mut inventory,
        breakdown_opt,
    ) in q_souls.iter_mut()
    {
        if let Some(expected_item) = task.expected_item() {
            let needs_item = task.requires_item_in_inventory();
            let expected_item_alive = q_entities.get(expected_item).is_ok();
            let has_expected = inventory.0 == Some(expected_item) && expected_item_alive;
            let has_mismatch = inventory.0.is_some() && !has_expected;
            let missing_required = needs_item && !has_expected;

            if has_mismatch || missing_required {
                unassign_task(
                    &mut commands,
                    soul_entity,
                    soul_transform.translation.truncate(),
                    &mut task,
                    &mut path,
                    Some(&mut inventory),
                    None,
                    &mut queries,
                    world_map.as_ref(),
                    true,
                );
                continue;
            }
        }

        let was_busy = !matches!(*task, AssignedTask::None);
        let old_work_type = task.work_type();
        let old_task_entity = task.get_target_entity();

        let mut ctx = TaskExecutionContext {
            soul_entity,
            soul_transform,
            soul: &mut soul,
            task: &mut task,
            dest: &mut dest,
            path: &mut path,
            inventory: &mut inventory,
            pf_context: &mut pf_context,
            queries: &mut queries,
        };

        run_task_handler(
            &mut ctx,
            &mut commands,
            &soul_handles,
            &time,
            world_map.as_ref(),
            breakdown_opt,
            &q_wheelbarrows,
        );

        if was_busy && matches!(*task, AssignedTask::None)
            && let Some(work_type) = old_work_type {
                commands.trigger(OnTaskCompleted {
                    entity: soul_entity,
                    task_entity: old_task_entity.unwrap_or(Entity::PLACEHOLDER),
                    work_type,
                });

                commands.entity(soul_entity).remove::<WorkingOn>();

                info!(
                    "EVENT: OnTaskCompleted triggered for Soul {:?}",
                    soul_entity
                );
            }
    }
}
