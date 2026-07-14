use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::OnTaskCompleted;
use hw_core::visual::SoulTaskHandles;
use hw_jobs::AssignedTask;
use hw_logistics::Wheelbarrow;
use hw_world::WorldMapRead;
use hw_world::pathfinding::PathfindingContext;

#[cfg(feature = "profiling")]
use crate::soul_ai::execute::task_execution::TaskExecutionPerfMetrics;
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
    #[cfg(feature = "profiling")] mut perf_metrics: ResMut<TaskExecutionPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    let mut souls_queried = 0u32;
    #[cfg(feature = "profiling")]
    let mut idle_skips = 0u32;
    #[cfg(feature = "profiling")]
    let mut handler_runs = 0u32;

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
        #[cfg(feature = "profiling")]
        {
            souls_queried = souls_queried.saturating_add(1);
        }

        // `&task` is an immutable reborrow of `Mut<AssignedTask>`. これを
        // TaskExecutionContext の `&mut AssignedTask` に渡す前に判定し、idle
        // Soul の5コンポーネントに不要な Changed を立てない。
        if is_idle_task(&task) {
            #[cfg(feature = "profiling")]
            {
                idle_skips = idle_skips.saturating_add(1);
            }
            continue;
        }

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

        #[cfg(feature = "profiling")]
        {
            handler_runs = handler_runs.saturating_add(1);
        }
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

    #[cfg(feature = "profiling")]
    {
        perf_metrics.souls_queried = perf_metrics.souls_queried.saturating_add(souls_queried);
        perf_metrics.idle_skips = perf_metrics.idle_skips.saturating_add(idle_skips);
        perf_metrics.handler_runs = perf_metrics.handler_runs.saturating_add(handler_runs);
    }
}

/// `Mut<AssignedTask>` を mutable に dereference せず、idle task を判定する。
fn is_idle_task(task: &AssignedTask) -> bool {
    matches!(task, AssignedTask::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::soul::{DamnedSoul, Destination, Path};
    use hw_jobs::{GeneratePowerData, GeneratePowerPhase};
    use hw_logistics::types::Inventory;

    fn idle_guard_probe_system(mut q_souls: TaskExecutionSoulQuery) {
        for (_, _, _, task, _, _, _, _) in q_souls.iter_mut() {
            if is_idle_task(&task) {
                continue;
            }
            unreachable!("the probe only spawns AssignedTask::None");
        }
    }

    #[derive(Resource, Default)]
    struct ActiveTaskProbe {
        reached_without_working_on: bool,
    }

    fn active_task_without_working_on_probe_system(
        mut q_souls: TaskExecutionSoulQuery,
        mut probe: ResMut<ActiveTaskProbe>,
    ) {
        for (_, _, _, task, _, _, _, _) in q_souls.iter_mut() {
            if !is_idle_task(&task) {
                probe.reached_without_working_on = true;
            }
        }
    }

    fn spawn_task_execution_soul(world: &mut World, task: AssignedTask) -> Entity {
        world
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                task,
                Destination(Vec2::ZERO),
                Path::default(),
                Inventory::default(),
            ))
            .id()
    }

    fn assert_component_unchanged<T: Component>(world: &mut World, entity: Entity) {
        let mut changed_components = world.query_filtered::<Entity, Changed<T>>();
        assert!(
            !changed_components
                .iter(world)
                .any(|changed| changed == entity),
            "{} was unexpectedly marked Changed",
            std::any::type_name::<T>()
        );
    }

    #[test]
    fn idle_guard_leaves_task_context_components_unchanged() {
        let mut world = World::new();
        let soul = spawn_task_execution_soul(&mut world, AssignedTask::None);
        world.clear_trackers();

        let mut schedule = Schedule::default();
        schedule.add_systems(idle_guard_probe_system);
        schedule.run(&mut world);

        assert_component_unchanged::<DamnedSoul>(&mut world, soul);
        assert_component_unchanged::<AssignedTask>(&mut world, soul);
        assert_component_unchanged::<Destination>(&mut world, soul);
        assert_component_unchanged::<Path>(&mut world, soul);
        assert_component_unchanged::<Inventory>(&mut world, soul);
    }

    #[test]
    fn active_task_without_working_on_remains_in_task_execution_query() {
        let mut world = World::new();
        world.init_resource::<ActiveTaskProbe>();
        spawn_task_execution_soul(
            &mut world,
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: Entity::PLACEHOLDER,
                tile_pos: Vec2::ZERO,
                phase: GeneratePowerPhase::GoingToTile,
            }),
        );
        world.clear_trackers();

        let mut schedule = Schedule::default();
        schedule.add_systems(active_task_without_working_on_probe_system);
        schedule.run(&mut world);

        assert!(
            world
                .resource::<ActiveTaskProbe>()
                .reached_without_working_on
        );
    }
}
