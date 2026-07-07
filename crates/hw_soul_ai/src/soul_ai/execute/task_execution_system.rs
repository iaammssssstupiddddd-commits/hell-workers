use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::OnTaskCompleted;
use hw_core::visual::SoulTaskHandles;
use hw_logistics::Wheelbarrow;
use hw_world::WorldMapRead;
use hw_world::pathfinding::PathfindingContext;

use crate::soul_ai::execute::task_execution::context::{
    TaskEndDisposition, TaskExecEnv, TaskExecutionContext, TaskQueries,
};
use crate::soul_ai::execute::task_execution::handler::dispatch::run_task_handler;
use crate::soul_ai::helpers::query_types::TaskExecutionSoulQuery;
use crate::soul_ai::helpers::work::unassign_task;

#[derive(SystemParam)]
pub struct TaskExecResources<'w, 's> {
    pub soul_handles: Res<'w, SoulTaskHandles>,
    pub time: Res<'w, Time>,
    pub world_map: WorldMapRead<'w>,
    pub pf_context: Local<'s, PathfindingContext>,
}

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: TaskExecutionSoulQuery,
    mut queries: TaskQueries,
    mut res: TaskExecResources,
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
                    crate::soul_ai::helpers::work::SoulDropCtx {
                        soul_entity,
                        drop_pos: soul_transform.translation.truncate(),
                        inventory: Some(&mut inventory),
                        dropped_item_res: None,
                    },
                    &mut task,
                    &mut path,
                    &mut queries,
                    res.world_map.as_ref(),
                    true,
                );
                continue;
            }
        }

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
            pf_context: &mut res.pf_context,
            queries: &mut queries,
            env: TaskExecEnv {
                soul_handles: &res.soul_handles,
                time: res.time.as_ref(),
                world_map: res.world_map.as_ref(),
                breakdown: breakdown_opt,
            },
            end_disposition: TaskEndDisposition::Running,
        };

        run_task_handler(&mut ctx, &mut commands, &q_wheelbarrows);

        let end_disposition = ctx.end_disposition;

        if end_disposition == TaskEndDisposition::Completed
            && let Some(work_type) = old_work_type
        {
            commands.trigger(OnTaskCompleted {
                entity: soul_entity,
                task_entity: old_task_entity.unwrap_or(Entity::PLACEHOLDER),
                work_type,
            });

            debug!(
                "EVENT: OnTaskCompleted triggered for Soul {:?}",
                soul_entity
            );
        }
    }
}
