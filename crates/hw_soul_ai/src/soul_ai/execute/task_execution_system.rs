use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::publish_task_completed;
use hw_core::relationships::WorkingOn;
use hw_core::visual::SoulTaskHandles;
use hw_jobs::{ActiveTaskIdentity, AssignedTask};
use hw_logistics::Wheelbarrow;
use hw_world::WorldMapRead;
use hw_world::pathfinding::PathfindingContext;

#[cfg(feature = "profiling")]
use crate::soul_ai::execute::task_execution::TaskExecutionPerfMetrics;
use crate::soul_ai::execute::task_execution::context::{
    TaskExecEnv, TaskExecutionContext, TaskHandlerControl, TaskQueries,
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
        identity_opt,
        working_on_opt,
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

        if !has_consistent_task_identity(identity_opt.as_deref(), working_on_opt) {
            let reason = if identity_opt.is_some() {
                "WorkingOn target differs from ActiveTaskIdentity"
            } else {
                "ActiveTaskIdentity is missing"
            };
            warn!(
                "TASK_EXEC: Soul {:?} retryably aborting task because {}",
                soul_entity, reason
            );
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
                false,
            );
            continue;
        }
        let Some(mut identity) = identity_opt else {
            unreachable!("identity consistency check requires ActiveTaskIdentity");
        };

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
                    false,
                );
                continue;
            }
        }

        let completed_identity = {
            let mut ctx = TaskExecutionContext {
                soul_entity,
                soul_transform,
                soul: &mut soul,
                task: &mut task,
                dest: &mut dest,
                path: &mut path,
                inventory: &mut inventory,
                identity: &mut identity,
                pf_context: &mut res.pf_context,
                queries: &mut queries,
                env: TaskExecEnv {
                    soul_handles: &res.soul_handles,
                    time: res.time.as_ref(),
                    world_map: res.world_map.as_ref(),
                    breakdown: breakdown_opt,
                },
                end_state: default(),
            };

            #[cfg(feature = "profiling")]
            {
                handler_runs = handler_runs.saturating_add(1);
            }
            let handler_control = run_task_handler(&mut ctx, &mut commands, &q_wheelbarrows);
            if handler_control == TaskHandlerControl::AlreadyEnded {
                debug!(
                    "TASK_EXEC: Soul {:?} handler attempted a duplicate terminal transition",
                    soul_entity
                );
            }

            if ctx.is_completed() {
                Some(ctx.task_identity())
            } else {
                None
            }
        };

        if let Some(identity) = completed_identity {
            publish_task_completed(
                &mut commands,
                soul_entity,
                identity.assignment_entity,
                identity.current_target_entity,
                identity.current_work_type,
            );

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

fn has_consistent_task_identity(
    identity: Option<&ActiveTaskIdentity>,
    working_on: Option<&WorkingOn>,
) -> bool {
    identity.is_some_and(|identity| identity.matches_working_on(working_on.map(|value| value.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{
        OnTaskAbandoned, OnTaskCompleted, ResourceReservationOp, ResourceReservationRequest,
        TaskCompletedVisualMessage,
    };
    use hw_core::soul::{DamnedSoul, Destination, Path};
    use hw_core::visual::SoulTaskHandles;
    use hw_jobs::{
        Blueprint, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
        BucketTransportSource, BuildData, BuildPhase, BuildingType, Designation, GeneratePowerData,
        GeneratePowerPhase, HaulData, HaulPhase, WorkType,
    };
    use hw_logistics::zone::Stockpile;
    use hw_logistics::{Inventory, ResourceItem, ResourceType, SharedResourceCache};
    use hw_world::WorldMap;

    #[derive(Resource, Default)]
    struct TaskNotificationReceipts {
        completed_domain: Vec<OnTaskCompleted>,
        completed_visual: Vec<TaskCompletedVisualMessage>,
        abandoned: Vec<OnTaskAbandoned>,
        reservation_ops: Vec<ResourceReservationOp>,
    }

    fn empty_soul_task_handles() -> SoulTaskHandles {
        SoulTaskHandles {
            wood: default(),
            tree_animes: Vec::new(),
            rock: default(),
            icon_bone_small: default(),
            icon_sand_small: default(),
            icon_stasis_mud_small: default(),
            bucket_water: default(),
            bucket_empty: default(),
        }
    }

    fn record_task_completed(
        trigger: On<OnTaskCompleted>,
        mut receipts: ResMut<TaskNotificationReceipts>,
    ) {
        receipts.completed_domain.push(*trigger.event());
    }

    fn collect_task_notification_messages(
        mut completed: MessageReader<TaskCompletedVisualMessage>,
        mut abandoned: MessageReader<OnTaskAbandoned>,
        mut reservations: MessageReader<ResourceReservationRequest>,
        mut receipts: ResMut<TaskNotificationReceipts>,
    ) {
        receipts.completed_visual.extend(completed.read().copied());
        receipts.abandoned.extend(abandoned.read().copied());
        receipts
            .reservation_ops
            .extend(reservations.read().map(|request| request.op.clone()));
    }

    fn task_execution_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldMap::default())
            .insert_resource(empty_soul_task_handles())
            .init_resource::<SharedResourceCache>()
            .init_resource::<TaskNotificationReceipts>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskCompletedVisualMessage>()
            .add_message::<OnTaskAbandoned>()
            .add_observer(record_task_completed)
            .add_systems(
                Update,
                (
                    task_execution_system,
                    ApplyDeferred,
                    collect_task_notification_messages,
                )
                    .chain(),
            );
        app
    }

    fn idle_guard_probe_system(mut q_souls: TaskExecutionSoulQuery) {
        for (_, _, _, task, _, _, _, _, _, _) in q_souls.iter_mut() {
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
        for (_, _, _, task, _, _, _, _, _, _) in q_souls.iter_mut() {
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
        let soul = spawn_task_execution_soul(
            &mut world,
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: Entity::PLACEHOLDER,
                tile_pos: Vec2::ZERO,
                phase: GeneratePowerPhase::GoingToTile,
            }),
        );
        world.entity_mut(soul).insert(ActiveTaskIdentity::new(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            WorkType::GeneratePower,
        ));
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

    #[test]
    fn identity_preflight_requires_identity_and_rejects_present_target_mismatch() {
        let mut world = World::new();
        let assignment = world.spawn_empty().id();
        let current_target = world.spawn_empty().id();
        let other_target = world.spawn_empty().id();
        let identity = ActiveTaskIdentity::new(assignment, current_target, WorkType::Chop);

        assert!(!has_consistent_task_identity(None, None));
        assert!(!has_consistent_task_identity(Some(&identity), None));
        assert!(has_consistent_task_identity(
            Some(&identity),
            Some(&WorkingOn(current_target))
        ));
        assert!(!has_consistent_task_identity(
            Some(&identity),
            Some(&WorkingOn(other_target))
        ));

        let mut detached = identity;
        detached.detach_from_working_on();
        assert!(has_consistent_task_identity(Some(&detached), None));
        assert!(!has_consistent_task_identity(
            Some(&detached),
            Some(&WorkingOn(current_target))
        ));
    }

    #[test]
    fn stockpile_reject_retryably_aborts_without_completion_or_abandonment_notifications() {
        let mut app = task_execution_test_app();
        let item = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Visible,
                ResourceItem(ResourceType::Wood),
            ))
            .id();
        let stockpile = app
            .world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 1,
                    resource_type: Some(ResourceType::Rock),
                },
            ))
            .id();
        let assignment = app.world_mut().spawn_empty().id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::Haul(HaulData {
                    item,
                    stockpile,
                    phase: HaulPhase::Dropping,
                }),
                Destination(Vec2::ZERO),
                Path::default(),
                Inventory(Some(item)),
                ActiveTaskIdentity::new(assignment, stockpile, WorkType::Haul),
                WorkingOn(stockpile),
            ))
            .id();

        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(receipts.completed_domain.is_empty());
        assert!(receipts.completed_visual.is_empty());
        assert!(receipts.abandoned.is_empty());
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
    }

    #[test]
    fn missing_identity_retryably_unassigns_without_completion_notification() {
        let mut app = task_execution_test_app();
        let target = app.world_mut().spawn_empty().id();
        let soul = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::Build(BuildData {
                blueprint: target,
                phase: BuildPhase::Done,
            }),
        );
        app.world_mut().entity_mut(soul).insert(WorkingOn(target));

        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(receipts.completed_domain.is_empty());
        assert!(receipts.completed_visual.is_empty());
        assert!(receipts.abandoned.is_empty());
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
    }

    #[test]
    fn vanished_blueprint_done_phase_aborts_without_completion_notification() {
        let mut app = task_execution_test_app();
        let target = app.world_mut().spawn_empty().id();
        let assignment = app.world_mut().spawn_empty().id();
        let soul = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::Build(BuildData {
                blueprint: target,
                phase: BuildPhase::Done,
            }),
        );
        app.world_mut().entity_mut(soul).insert((
            ActiveTaskIdentity::new(assignment, target, WorkType::Build),
            WorkingOn(target),
        ));

        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(receipts.completed_domain.is_empty());
        assert!(receipts.completed_visual.is_empty());
        assert!(receipts.abandoned.is_empty());
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
    }

    #[test]
    fn bucket_abort_releases_active_reservations_without_terminal_notifications() {
        let mut app = task_execution_test_app();
        let bucket = app.world_mut().spawn_empty().id();
        let tank = app.world_mut().spawn_empty().id();
        let mixer = app.world_mut().spawn_empty().id();
        let assignment = app.world_mut().spawn_empty().id();
        let soul = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::BucketTransport(BucketTransportData {
                bucket,
                source: BucketTransportSource::Tank {
                    tank,
                    needs_fill: true,
                },
                destination: BucketTransportDestination::Mixer(mixer),
                amount: 0,
                phase: BucketTransportPhase::GoingToBucket,
            }),
        );
        app.world_mut().entity_mut(soul).insert((
            ActiveTaskIdentity::new(assignment, mixer, WorkType::HaulWaterToMixer),
            WorkingOn(mixer),
        ));

        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(receipts.completed_domain.is_empty());
        assert!(receipts.completed_visual.is_empty());
        assert!(receipts.abandoned.is_empty());
        assert_eq!(
            receipts.reservation_ops,
            vec![
                ResourceReservationOp::ReleaseSource {
                    source: bucket,
                    amount: 1,
                },
                ResourceReservationOp::ReleaseSource {
                    source: tank,
                    amount: 1,
                },
                ResourceReservationOp::ReleaseMixerDestination {
                    target: mixer,
                    resource_type: ResourceType::Water,
                },
            ]
        );
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
    }

    #[test]
    fn normal_completion_publishes_matching_assignment_and_current_identity() {
        let mut app = task_execution_test_app();
        let assignment = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn((
                Transform::default(),
                Blueprint::new(BuildingType::Floor, vec![(1, 1)]),
            ))
            .id();
        let soul = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::Build(BuildData {
                blueprint: target,
                phase: BuildPhase::Done,
            }),
        );
        app.world_mut().entity_mut(soul).insert((
            ActiveTaskIdentity::new(assignment, target, WorkType::Build),
            WorkingOn(target),
        ));

        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        let expected = OnTaskCompleted {
            entity: soul,
            assignment_entity: assignment,
            current_target_entity: target,
            current_work_type: WorkType::Build,
        };
        assert_eq!(receipts.completed_domain.as_slice(), &[expected]);
        assert_eq!(
            receipts.completed_visual.as_slice(),
            &[TaskCompletedVisualMessage {
                entity: soul,
                assignment_entity: assignment,
                current_target_entity: target,
                current_work_type: WorkType::Build,
            }]
        );
        assert!(receipts.abandoned.is_empty());
    }

    #[test]
    fn building_progress_completion_finishes_without_a_follow_up_done_frame() {
        let mut app = task_execution_test_app();
        let assignment = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn((
                Transform::default(),
                Blueprint::new(BuildingType::Floor, vec![(0, 0)]),
                Designation {
                    work_type: WorkType::Build,
                },
            ))
            .id();
        let soul = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::Build(BuildData {
                blueprint: target,
                phase: BuildPhase::Building { progress: 1.0 },
            }),
        );
        app.world_mut().entity_mut(soul).insert((
            ActiveTaskIdentity::new(assignment, target, WorkType::Build),
            WorkingOn(target),
        ));
        app.world_mut()
            .entity_mut(soul)
            .get_mut::<Transform>()
            .expect("task execution soul has Transform")
            .translation = WorldMap::grid_to_world(1, 0).extend(0.0);
        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert_eq!(
            receipts.completed_domain.as_slice(),
            &[OnTaskCompleted {
                entity: soul,
                assignment_entity: assignment,
                current_target_entity: target,
                current_work_type: WorkType::Build,
            }]
        );
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    }

    #[test]
    fn chain_completion_preserves_root_assignment_and_publishes_final_identity() {
        let mut app = task_execution_test_app();
        let assignment = app.world_mut().spawn_empty().id();
        let initial_target = app.world_mut().spawn_empty().id();
        let final_target = app
            .world_mut()
            .spawn((
                Transform::default(),
                Blueprint::new(BuildingType::Floor, vec![(1, 1)]),
            ))
            .id();
        let mut identity = ActiveTaskIdentity::new(assignment, initial_target, WorkType::Chop);
        identity.transition_to(final_target, WorkType::Build);
        let soul = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::Build(BuildData {
                blueprint: final_target,
                phase: BuildPhase::Done,
            }),
        );
        app.world_mut()
            .entity_mut(soul)
            .insert((identity, WorkingOn(final_target)));

        app.update();

        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert_eq!(
            receipts.completed_domain.as_slice(),
            &[OnTaskCompleted {
                entity: soul,
                assignment_entity: assignment,
                current_target_entity: final_target,
                current_work_type: WorkType::Build,
            }]
        );
        assert_eq!(
            receipts.completed_visual.as_slice(),
            &[TaskCompletedVisualMessage {
                entity: soul,
                assignment_entity: assignment,
                current_target_entity: final_target,
                current_work_type: WorkType::Build,
            }]
        );
        assert!(receipts.abandoned.is_empty());
    }
}
