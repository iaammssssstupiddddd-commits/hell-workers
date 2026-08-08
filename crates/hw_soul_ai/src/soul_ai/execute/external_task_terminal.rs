//! Exact-identity task terminalization for root-owned world transactions.

use std::collections::HashSet;

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use hw_core::events::publish_task_completed;
use hw_core::relationships::WorkingOn;
use hw_core::soul::Path;
use hw_jobs::{ActiveTaskIdentity, AssignedTask, DeconstructPhase};
use hw_logistics::Inventory;
use hw_world::WorldMap;

use crate::soul_ai::execute::task_execution::TaskUnassignQueries;
use crate::soul_ai::helpers::work::{
    SoulDropCtx, unassign_task, unassign_task_preserving_wheelbarrow_cargo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactTaskTerminalDisposition {
    Complete,
    Abort { emit_abandoned: bool },
    AbortPreservingWheelbarrowCargo { emit_abandoned: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactTaskExpectation {
    References(Entity),
    DeconstructionAwaitingCommit { order: Entity, target: Entity },
}

impl ExactTaskExpectation {
    fn matches(
        self,
        task: &AssignedTask,
        identity: &ActiveTaskIdentity,
        working_on: Option<&WorkingOn>,
    ) -> bool {
        match self {
            Self::References(entity) => {
                task.references_entity(entity)
                    || identity.assignment_entity == entity
                    || identity.current_target_entity == entity
                    || working_on.is_some_and(|working_on| working_on.0 == entity)
            }
            Self::DeconstructionAwaitingCommit { order, target } => matches!(
                task,
                AssignedTask::Deconstruct(data)
                    if data.order == order
                        && data.target == target
                        && data.phase == DeconstructPhase::AwaitingCommit
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactTaskTerminalRequest {
    pub worker: Entity,
    pub expected_identity: ActiveTaskIdentity,
    pub expectation: ExactTaskExpectation,
    pub disposition: ExactTaskTerminalDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactTaskTerminalResult {
    Applied,
    /// This request was valid, but another request made the all-or-none batch
    /// fail validation, so no task in the batch was changed.
    BatchAborted,
    DuplicateWorker,
    MissingWorker,
    IdentityMismatch,
    TaskMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactTaskTerminalOutcome {
    pub worker: Entity,
    pub result: ExactTaskTerminalResult,
}

type ExactTaskTerminalSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static Transform>,
        &'static mut AssignedTask,
        Option<&'static mut Path>,
        Option<&'static mut Inventory>,
        &'static ActiveTaskIdentity,
        Option<&'static WorkingOn>,
    ),
>;

/// Applies a batch synchronously so relationship cleanup is visible before a
/// root owner despawns its order or target in the same transaction. Validation
/// is all-or-none: if any request is stale or malformed, no worker is changed.
/// An exact shell may already have lost its Soul marker or `WorkingOn` edge.
/// A missing Transform is repairable only for Deconstruct, whose task owns no
/// carried resource or reservation that would require a fabricated drop point.
pub fn terminalize_exact_tasks(
    world: &mut World,
    requests: &[ExactTaskTerminalRequest],
) -> Vec<ExactTaskTerminalOutcome> {
    let mut seen_workers = HashSet::with_capacity(requests.len());
    let has_duplicate_worker = requests
        .iter()
        .any(|request| !seen_workers.insert(request.worker));
    let mut outcomes = requests
        .iter()
        .map(|request| ExactTaskTerminalOutcome {
            worker: request.worker,
            result: validate_exact_task(world, request),
        })
        .collect::<Vec<_>>();
    if has_duplicate_worker {
        for outcome in &mut outcomes {
            outcome.result = if requests
                .iter()
                .filter(|request| request.worker == outcome.worker)
                .count()
                > 1
            {
                ExactTaskTerminalResult::DuplicateWorker
            } else {
                ExactTaskTerminalResult::BatchAborted
            };
        }
        return outcomes;
    }
    if outcomes
        .iter()
        .any(|outcome| outcome.result != ExactTaskTerminalResult::Applied)
    {
        for outcome in &mut outcomes {
            if outcome.result == ExactTaskTerminalResult::Applied {
                outcome.result = ExactTaskTerminalResult::BatchAborted;
            }
        }
        return outcomes;
    }

    let mut state: SystemState<(
        Commands,
        ExactTaskTerminalSoulQuery,
        TaskUnassignQueries,
        Res<WorldMap>,
    )> = SystemState::new(world);
    {
        let (mut commands, mut q_souls, mut queries, world_map) = state
            .get_mut(world)
            .expect("exact task terminal system state must validate");
        for request in requests {
            let (worker, transform, mut task, mut path, mut inventory, _, _) = q_souls
                .get_mut(request.worker)
                .expect("exclusive exact-task batch changed after successful preflight");
            let mut repaired_path = Path::default();
            let path_was_missing = path.is_none();
            let emit_abandoned = matches!(
                request.disposition,
                ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: true
                } | ExactTaskTerminalDisposition::AbortPreservingWheelbarrowCargo {
                    emit_abandoned: true
                }
            );
            let path = path.as_deref_mut().unwrap_or(&mut repaired_path);
            if let Some(transform) = transform {
                let inventory = if inventory
                    .as_ref()
                    .is_some_and(|inventory| inventory.0.is_some())
                {
                    inventory.as_deref_mut()
                } else {
                    None
                };
                let drop_ctx = SoulDropCtx {
                    soul_entity: worker,
                    drop_pos: transform.translation.truncate(),
                    inventory,
                    dropped_item_res: None,
                };
                if matches!(
                    request.disposition,
                    ExactTaskTerminalDisposition::AbortPreservingWheelbarrowCargo { .. }
                ) {
                    unassign_task_preserving_wheelbarrow_cargo(
                        &mut commands,
                        drop_ctx,
                        &mut task,
                        path,
                        &mut queries,
                        &world_map,
                        emit_abandoned,
                    );
                } else {
                    unassign_task(
                        &mut commands,
                        drop_ctx,
                        &mut task,
                        path,
                        &mut queries,
                        &world_map,
                        emit_abandoned,
                    );
                }
            } else {
                // Deconstruction never owns carried resources or reservations.
                // A corrupt shell without Transform can therefore release its
                // exact assignment without inventing a drop position.
                debug_assert!(matches!(*task, AssignedTask::Deconstruct(_)));
                if emit_abandoned {
                    commands.write_message(hw_core::events::OnTaskAbandoned { entity: worker });
                }
                *task = AssignedTask::None;
                path.waypoints.clear();
                commands
                    .entity(worker)
                    .remove::<(ActiveTaskIdentity, WorkingOn)>();
            }
            if request.disposition == ExactTaskTerminalDisposition::Complete {
                publish_task_completed(
                    &mut commands,
                    worker,
                    request.expected_identity.assignment_entity,
                    request.expected_identity.current_target_entity,
                    request.expected_identity.current_work_type,
                );
            }
            if path_was_missing {
                commands.entity(worker).try_insert(repaired_path);
            }
        }
    }
    state.apply(world);
    outcomes
}

fn validate_exact_task(
    world: &World,
    request: &ExactTaskTerminalRequest,
) -> ExactTaskTerminalResult {
    let Some(task) = world.get::<AssignedTask>(request.worker) else {
        return ExactTaskTerminalResult::MissingWorker;
    };
    let Some(identity) = world.get::<ActiveTaskIdentity>(request.worker) else {
        return ExactTaskTerminalResult::MissingWorker;
    };
    let working_on = world.get::<WorkingOn>(request.worker);
    if *identity != request.expected_identity
        || working_on.is_some_and(|working_on| !identity.matches_working_on(Some(working_on.0)))
        || task.work_type() != Some(identity.current_work_type)
    {
        ExactTaskTerminalResult::IdentityMismatch
    } else if !request.expectation.matches(task, identity, working_on)
        || (matches!(
            request.disposition,
            ExactTaskTerminalDisposition::AbortPreservingWheelbarrowCargo { .. }
        ) && !matches!(task, AssignedTask::HaulWithWheelbarrow(_)))
    {
        ExactTaskTerminalResult::TaskMismatch
    } else if world.get::<Transform>(request.worker).is_none()
        && !matches!(task, AssignedTask::Deconstruct(_))
    {
        ExactTaskTerminalResult::MissingWorker
    } else {
        ExactTaskTerminalResult::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{
        OnTaskAbandoned, ResourceReservationRequest, TaskCompletedVisualMessage,
    };
    use hw_core::logistics::WheelbarrowDestination;
    use hw_core::relationships::{DeliveringTo, LoadedIn, ParkedAt, PushedBy, TaskWorkers};
    use hw_core::soul::DamnedSoul;
    use hw_jobs::{
        AssignedTask, DeconstructData, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase, WorkType,
    };
    use hw_logistics::{BelongsTo, ResourceItem, ResourceType, SharedResourceCache, Wheelbarrow};

    fn test_world() -> World {
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        world.init_resource::<SharedResourceCache>();
        world.init_resource::<Messages<ResourceReservationRequest>>();
        world.init_resource::<Messages<TaskCompletedVisualMessage>>();
        world.init_resource::<Messages<OnTaskAbandoned>>();
        world
    }

    fn spawn_worker(
        world: &mut World,
        order: Entity,
        target: Entity,
    ) -> (Entity, ActiveTaskIdentity) {
        let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
        let worker = world
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::Deconstruct(DeconstructData {
                    order,
                    target,
                    phase: DeconstructPhase::AwaitingCommit,
                }),
                Path::default(),
                Inventory::default(),
                identity,
                WorkingOn(order),
            ))
            .id();
        world.flush();
        (worker, identity)
    }

    #[test]
    fn matching_identity_completes_once_and_cleans_relationship() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (worker, identity) = spawn_worker(&mut world, order, target);
        assert_eq!(
            world.get::<TaskWorkers>(order).map(TaskWorkers::len),
            Some(1)
        );

        let request = ExactTaskTerminalRequest {
            worker,
            expected_identity: identity,
            expectation: ExactTaskExpectation::DeconstructionAwaitingCommit { order, target },
            disposition: ExactTaskTerminalDisposition::Complete,
        };
        assert_eq!(
            terminalize_exact_tasks(&mut world, &[request])[0].result,
            ExactTaskTerminalResult::Applied
        );
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(world.get::<WorkingOn>(worker).is_none());
        assert!(world.get::<ActiveTaskIdentity>(worker).is_none());
        assert!(
            world
                .get::<TaskWorkers>(order)
                .is_none_or(TaskWorkers::is_empty)
        );
        assert_eq!(
            terminalize_exact_tasks(&mut world, &[request])[0].result,
            ExactTaskTerminalResult::MissingWorker
        );
    }

    #[test]
    fn stale_identity_does_not_touch_the_worker() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (worker, _) = spawn_worker(&mut world, order, target);
        let stale_assignment = world.spawn_empty().id();

        let outcome = terminalize_exact_tasks(
            &mut world,
            &[ExactTaskTerminalRequest {
                worker,
                expected_identity: ActiveTaskIdentity::new(
                    stale_assignment,
                    stale_assignment,
                    WorkType::Deconstruct,
                ),
                expectation: ExactTaskExpectation::References(target),
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            }],
        );

        assert_eq!(outcome[0].result, ExactTaskTerminalResult::IdentityMismatch);
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::Deconstruct(_))
        ));
        assert_eq!(
            world.get::<WorkingOn>(worker).map(|working| working.0),
            Some(order)
        );
    }

    #[test]
    fn preserving_wheelbarrow_abort_keeps_loaded_items_attached() {
        let mut world = test_world();
        let parking = world.spawn_empty().id();
        let destination = world.spawn_empty().id();
        let assignment = world.spawn_empty().id();
        let wheelbarrow = world
            .spawn((
                ResourceItem(ResourceType::Wheelbarrow),
                Wheelbarrow { capacity: 8 },
                BelongsTo(parking),
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        let loaded_transform = Transform::from_xyz(123.0, 456.0, 0.0);
        let item = world
            .spawn((
                ResourceItem(ResourceType::Sand),
                LoadedIn(wheelbarrow),
                DeliveringTo(destination),
                loaded_transform,
                Visibility::Hidden,
            ))
            .id();
        let identity = ActiveTaskIdentity::new(assignment, assignment, WorkType::WheelbarrowHaul);
        let worker = world
            .spawn((
                Transform::from_xyz(32.0, 64.0, 0.0),
                DamnedSoul::default(),
                AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    wheelbarrow,
                    source_pos: Vec2::ZERO,
                    destination: WheelbarrowDestination::Stockpile(destination),
                    collect_source: None,
                    collect_amount: 0,
                    collect_resource_type: None,
                    items: vec![item],
                    phase: HaulWithWheelbarrowPhase::GoingToDestination,
                }),
                Path::default(),
                Inventory(Some(wheelbarrow)),
                identity,
                WorkingOn(assignment),
            ))
            .id();
        world.entity_mut(wheelbarrow).insert(PushedBy(worker));
        world.flush();

        let outcome = terminalize_exact_tasks(
            &mut world,
            &[ExactTaskTerminalRequest {
                worker,
                expected_identity: identity,
                expectation: ExactTaskExpectation::References(wheelbarrow),
                disposition: ExactTaskTerminalDisposition::AbortPreservingWheelbarrowCargo {
                    emit_abandoned: false,
                },
            }],
        );

        assert_eq!(outcome[0].result, ExactTaskTerminalResult::Applied);
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert_eq!(world.get::<Inventory>(worker).unwrap().0, None);
        assert!(world.get::<WorkingOn>(worker).is_none());
        assert!(world.get::<ActiveTaskIdentity>(worker).is_none());
        assert_eq!(
            world.get::<LoadedIn>(item).map(|loaded| loaded.0),
            Some(wheelbarrow)
        );
        assert_eq!(world.get::<Visibility>(item), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Transform>(item), Some(&loaded_transform));
        assert!(world.get::<DeliveringTo>(item).is_none());
        assert!(world.get::<PushedBy>(wheelbarrow).is_none());
        assert_eq!(
            world.get::<ParkedAt>(wheelbarrow).map(|parked| parked.0),
            Some(parking)
        );
    }

    #[test]
    fn one_invalid_request_aborts_the_entire_batch_without_mutation() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (valid_worker, valid_identity) = spawn_worker(&mut world, order, target);
        let (invalid_worker, invalid_identity) = spawn_worker(&mut world, order, target);
        world
            .entity_mut(invalid_worker)
            .remove::<ActiveTaskIdentity>();

        let requests = [
            ExactTaskTerminalRequest {
                worker: valid_worker,
                expected_identity: valid_identity,
                expectation: ExactTaskExpectation::References(order),
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            },
            ExactTaskTerminalRequest {
                worker: invalid_worker,
                expected_identity: invalid_identity,
                expectation: ExactTaskExpectation::References(order),
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            },
        ];

        let outcomes = terminalize_exact_tasks(&mut world, &requests);

        assert_eq!(outcomes[0].result, ExactTaskTerminalResult::BatchAborted);
        assert_eq!(outcomes[1].result, ExactTaskTerminalResult::MissingWorker);
        assert!(matches!(
            world.get::<AssignedTask>(valid_worker),
            Some(AssignedTask::Deconstruct(_))
        ));
        assert_eq!(
            world
                .get::<WorkingOn>(valid_worker)
                .map(|working| working.0),
            Some(order)
        );
    }

    #[test]
    fn missing_runtime_path_is_repaired_while_terminalizing() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (worker, identity) = spawn_worker(&mut world, order, target);
        world.entity_mut(worker).remove::<Path>();

        let outcomes = terminalize_exact_tasks(
            &mut world,
            &[ExactTaskTerminalRequest {
                worker,
                expected_identity: identity,
                expectation: ExactTaskExpectation::References(order),
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            }],
        );

        assert_eq!(outcomes[0].result, ExactTaskTerminalResult::Applied);
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(world.get::<Path>(worker).is_some());
        assert!(world.get::<WorkingOn>(worker).is_none());
        assert!(world.get::<ActiveTaskIdentity>(worker).is_none());
    }

    #[test]
    fn missing_soul_marker_does_not_strand_an_exact_deconstruction_shell() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (worker, identity) = spawn_worker(&mut world, order, target);
        world.entity_mut(worker).remove::<DamnedSoul>();

        let outcome = terminalize_exact_tasks(
            &mut world,
            &[ExactTaskTerminalRequest {
                worker,
                expected_identity: identity,
                expectation: ExactTaskExpectation::DeconstructionAwaitingCommit { order, target },
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            }],
        );

        assert_eq!(outcome[0].result, ExactTaskTerminalResult::Applied);
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(world.get::<WorkingOn>(worker).is_none());
        assert!(world.get::<ActiveTaskIdentity>(worker).is_none());
        assert!(
            world
                .get::<TaskWorkers>(order)
                .is_none_or(TaskWorkers::is_empty)
        );
    }

    #[test]
    fn missing_transform_releases_deconstruction_without_fabricating_a_position() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (worker, identity) = spawn_worker(&mut world, order, target);
        world.entity_mut(worker).remove::<Transform>();

        let outcome = terminalize_exact_tasks(
            &mut world,
            &[ExactTaskTerminalRequest {
                worker,
                expected_identity: identity,
                expectation: ExactTaskExpectation::DeconstructionAwaitingCommit { order, target },
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            }],
        );

        assert_eq!(outcome[0].result, ExactTaskTerminalResult::Applied);
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(world.get::<Transform>(worker).is_none());
        assert!(world.get::<WorkingOn>(worker).is_none());
        assert!(world.get::<ActiveTaskIdentity>(worker).is_none());
        assert!(
            world
                .get::<TaskWorkers>(order)
                .is_none_or(TaskWorkers::is_empty)
        );
    }

    #[test]
    fn missing_working_relationship_does_not_strand_an_exact_worker_shell() {
        let mut world = test_world();
        let order = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let (worker, identity) = spawn_worker(&mut world, order, target);
        world.entity_mut(worker).remove::<WorkingOn>();

        let outcome = terminalize_exact_tasks(
            &mut world,
            &[ExactTaskTerminalRequest {
                worker,
                expected_identity: identity,
                expectation: ExactTaskExpectation::DeconstructionAwaitingCommit { order, target },
                disposition: ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            }],
        );

        assert_eq!(outcome[0].result, ExactTaskTerminalResult::Applied);
        assert!(matches!(
            world.get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(world.get::<ActiveTaskIdentity>(worker).is_none());
    }
}
