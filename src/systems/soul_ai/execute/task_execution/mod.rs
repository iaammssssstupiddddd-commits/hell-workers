//! タスク実行モジュール
//!
//! コア実装は hw_ai に移設済み。このモジュールは後方互換 re-export と
//! WorldMapRead/unassign_task に依存するシステム関数を保持する。

// 後方互換のための型/モジュール re-export
pub mod common;
pub mod context;
pub mod handler;
pub mod move_plant;
pub mod transport_common;
pub mod types;

pub use types::AssignedTask;

// apply_task_assignment_requests_system は hw_ai に移設済み
pub use hw_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;

use crate::events::OnTaskCompleted;
use crate::systems::soul_ai::helpers::query_types::TaskExecutionSoulQuery;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMapRead;
use bevy::prelude::*;
use hw_core::visual::SoulTaskHandles;

use context::TaskExecutionContext;
use handler::run_task_handler;

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: TaskExecutionSoulQuery,
    mut queries: context::TaskQueries,
    soul_handles: Res<SoulTaskHandles>,
    time: Res<Time>,
    world_map: WorldMapRead,
    mut pf_context: Local<crate::world::pathfinding::PathfindingContext>,
    q_wheelbarrows: Query<
        (&Transform, Option<&crate::relationships::ParkedAt>),
        With<crate::systems::logistics::Wheelbarrow>,
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
            pf_context: &mut *pf_context,
            queries: &mut queries,
        };

        run_task_handler(
            &mut ctx,
            &mut commands,
            &soul_handles,
            &time,
            world_map.as_ref(),
            breakdown_opt.as_deref(),
            &q_wheelbarrows,
        );

        if was_busy && matches!(*task, AssignedTask::None) {
            if let Some(work_type) = old_work_type {
                commands.trigger(OnTaskCompleted {
                    entity: soul_entity,
                    task_entity: old_task_entity.unwrap_or(Entity::PLACEHOLDER),
                    work_type,
                });

                commands
                    .entity(soul_entity)
                    .remove::<crate::relationships::WorkingOn>();

                info!(
                    "EVENT: OnTaskCompleted triggered for Soul {:?}",
                    soul_entity
                );
            }
        }
    }
}
