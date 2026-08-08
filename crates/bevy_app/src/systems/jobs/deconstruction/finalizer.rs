use std::collections::{HashMap, HashSet};
#[cfg(feature = "profiling")]
use std::time::Instant;

use bevy::prelude::*;
use hw_core::WorldEpoch;
use hw_core::relationships::WorkingOn;
use hw_core::soul::DamnedSoul;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    SoulSpaPhase, SoulSpaSite, SoulSpaTile,
};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, Building, BuildingType, DeconstructPhase,
    DeconstructionBlockReason, DeconstructionBlocker, DeconstructionCancelOutcome,
    DeconstructionCancelRequest, DeconstructionCancelResult, DeconstructionCommitClaim,
    DeconstructionCommitOutcome, DeconstructionCommitRequest, DeconstructionCommitResult,
    DeconstructionOrder, DeconstructionPending, Designation, MovePlanned, MovePlantTask,
    PendingBuildingMove, TargetDeconstructionRoot, TaskDiagnosticDomainMask,
    TaskDiagnosticInputRevisions, WorkType, deconstruction_salvage, resolve_deconstruction_target,
    supports_deconstruction_cleanup,
};
use hw_logistics::SharedResourceCache;
use hw_logistics::construction_helpers::ResourceItemVisualHandles;
use hw_logistics::transport_request::{
    OwnerTransportCleanupResult, close_transport_requests_for_removed_owners,
    transport_requests_referencing_removed_owners,
};
use hw_soul_ai::{
    ExactTaskExpectation, ExactTaskTerminalDisposition, ExactTaskTerminalRequest,
    ExactTaskTerminalResult, RestAreaReleaseResult, release_rest_area_for_removed_owner,
    rest_area_relationship_sources, terminalize_exact_tasks,
};
use hw_visual::Building3dVisual;
use hw_world::map::WorldMapOwnerSnapshot;
use hw_world::{RoomDetectionState, WorldMap};

use crate::systems::energy::grid_recalc::EnergyUpdateDirty;

use super::designation::designation_target_shape_is_supported;
use super::recovery::{
    FacilityRecoveryPlan, RecoveryPlanFailure, apply_facility_recovery, prepare_facility_recovery,
    recovery_plan_still_matches,
};
use crate::systems::jobs::exact_task_cleanup::{CompletingExactTask, prepare_owner_task_terminals};

#[derive(Debug)]
struct PreparedCommit {
    kind: BuildingType,
    owner_snapshot: WorldMapOwnerSnapshot,
    soul_spa_tiles: Vec<SoulSpaTileSnapshot>,
    power_topology: PowerTopologySnapshot,
    visual_entities: Vec<Entity>,
    recovery: FacilityRecoveryPlan,
    removed_owners: Vec<Entity>,
    terminal_requests: Vec<ExactTaskTerminalRequest>,
    transport_requests: Vec<Entity>,
    orders_to_despawn: Vec<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SoulSpaTileSnapshot {
    entity: Entity,
    grid: (i32, i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PowerTopologySnapshot {
    generator_grid: Option<Entity>,
    consumer_grid: Option<Entity>,
}

#[derive(Debug, Clone, Copy)]
struct CommitFailure {
    result: DeconstructionCommitResult,
    blocker: Option<(DeconstructionBlockReason, TaskDiagnosticDomainMask)>,
    discard_orphaned_order: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelatedOrdersFailure {
    CanonicalInvalid,
    MalformedSibling,
}

/// Profiling-only evidence for request-driven deconstruction transactions.
///
/// The production finalizer does not allocate or update these counters. A
/// validation pass is entered only for a current-world, non-canceled,
/// non-duplicate commit request, so an empty queue cannot produce a hidden
/// entity scan through this accounting path.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeconstructionPerfMetrics {
    /// Actual calls into owner validation for a commit request.
    pub(crate) commit_validation_passes: u64,
    /// Successfully applied owner cleanup transactions.
    pub(crate) successful_cleanup_transactions: u64,
    /// Recovery items actually spawned by successful transactions.
    pub(crate) recovery_items_spawned: u64,
    /// Monotonic elapsed nanoseconds spent in successful validation + apply.
    ///
    /// This deliberately excludes time between the driver writing a request
    /// and the finalizer consuming it on a later update.
    pub(crate) successful_transaction_elapsed_ns: u128,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SuccessfulCommitMetrics {
    #[cfg(feature = "profiling")]
    recovery_items_spawned: u64,
}

impl CommitFailure {
    const fn untouched(result: DeconstructionCommitResult) -> Self {
        Self {
            result,
            blocker: None,
            discard_orphaned_order: false,
        }
    }

    const fn blocked(
        result: DeconstructionCommitResult,
        reason: DeconstructionBlockReason,
        domains: TaskDiagnosticDomainMask,
    ) -> Self {
        Self {
            result,
            blocker: Some((reason, domains)),
            discard_orphaned_order: false,
        }
    }

    const fn orphaned_order(result: DeconstructionCommitResult) -> Self {
        Self {
            result,
            blocker: None,
            discard_orphaned_order: true,
        }
    }
}

/// Serializes cancel and commit requests into one owner-safe world transaction.
pub fn deconstruction_finalizer_system(world: &mut World) {
    let mut cancels: Vec<_> = world
        .resource_mut::<Messages<DeconstructionCancelRequest>>()
        .drain()
        .collect();
    let mut commits: Vec<_> = world
        .resource_mut::<Messages<DeconstructionCommitRequest>>()
        .drain()
        .collect();
    cancels.sort_unstable_by_key(|request| (request.world_epoch, request.order.to_bits()));
    commits.sort_unstable_by_key(|request| {
        (
            request.world_epoch,
            request.target.to_bits(),
            request.order.to_bits(),
            request.worker.to_bits(),
            request.identity.assignment_entity.to_bits(),
            request.identity.current_target_entity.to_bits(),
            request.identity.current_work_type.stable_index(),
            request.identity.binding_stable_index(),
        )
    });

    let current_epoch = world
        .get_resource::<WorldEpoch>()
        .copied()
        .unwrap_or_default()
        .get();
    let mut canceled_orders = HashMap::<Entity, Entity>::new();
    for request in cancels {
        process_cancel(world, current_epoch, request, &mut canceled_orders);
    }

    let mut committed_targets = HashSet::<Entity>::new();
    for request in commits {
        if request.world_epoch != current_epoch {
            write_commit_outcome(
                world,
                DeconstructionCommitOutcome {
                    result: DeconstructionCommitResult::StaleWorld,
                    ..commit_outcome_base(request)
                },
            );
            continue;
        }
        if let Some(&target) = canceled_orders.get(&request.order) {
            terminalize_commit_worker(
                world,
                request,
                ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            );
            write_commit_outcome(
                world,
                DeconstructionCommitOutcome {
                    target,
                    result: DeconstructionCommitResult::Canceled,
                    ..commit_outcome_base(request)
                },
            );
            continue;
        }
        if committed_targets.contains(&request.target) {
            terminalize_commit_worker(
                world,
                request,
                ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                },
            );
            write_commit_outcome(
                world,
                DeconstructionCommitOutcome {
                    result: DeconstructionCommitResult::Duplicate,
                    ..commit_outcome_base(request)
                },
            );
            continue;
        }

        #[cfg(feature = "profiling")]
        let transaction_started_at = Instant::now();
        #[cfg(feature = "profiling")]
        record_commit_validation_pass(world);
        match validate_commit(world, current_epoch, request) {
            Ok(prepared) => match commit_deconstruction(world, request, prepared) {
                Ok(_commit_metrics) => {
                    committed_targets.insert(request.target);
                    #[cfg(feature = "profiling")]
                    record_successful_commit_metrics(
                        world,
                        _commit_metrics,
                        transaction_started_at.elapsed(),
                    );
                    write_commit_outcome(
                        world,
                        DeconstructionCommitOutcome {
                            result: DeconstructionCommitResult::Committed,
                            ..commit_outcome_base(request)
                        },
                    );
                }
                Err(failure) => {
                    handle_commit_failure(world, request, failure);
                    write_commit_outcome(
                        world,
                        DeconstructionCommitOutcome {
                            result: failure.result,
                            ..commit_outcome_base(request)
                        },
                    );
                }
            },
            Err(failure) => {
                handle_commit_failure(world, request, failure);
                write_commit_outcome(
                    world,
                    DeconstructionCommitOutcome {
                        result: failure.result,
                        ..commit_outcome_base(request)
                    },
                );
            }
        }
    }
    world.flush();
}

fn process_cancel(
    world: &mut World,
    current_epoch: u64,
    request: DeconstructionCancelRequest,
    canceled_orders: &mut HashMap<Entity, Entity>,
) {
    if request.world_epoch != current_epoch {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: None,
                result: DeconstructionCancelResult::StaleWorld,
            },
        );
        return;
    }

    let Some(target) = world
        .get::<TargetDeconstructionRoot>(request.order)
        .map(|relation| relation.0)
    else {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: None,
                result: DeconstructionCancelResult::StaleOrder,
            },
        );
        return;
    };
    if world.get::<DeconstructionOrder>(request.order).is_none()
        || world
            .get::<Designation>(request.order)
            .is_none_or(|designation| designation.work_type != WorkType::Deconstruct)
        || world
            .get::<DeconstructionPending>(target)
            .is_none_or(|pending| pending.order != request.order)
    {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: Some(target),
                result: DeconstructionCancelResult::StaleOrder,
            },
        );
        return;
    }
    let Ok(orders_to_cancel) =
        related_deconstruction_orders_for_target(world, target, request.order)
    else {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: Some(target),
                result: DeconstructionCancelResult::StaleOrder,
            },
        );
        return;
    };
    if world.get::<DeconstructionCommitClaim>(target).is_some() {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: Some(target),
                result: DeconstructionCancelResult::ClaimInProgress,
            },
        );
        return;
    }

    let Ok(terminal_requests) = prepare_owner_task_terminals(world, &orders_to_cancel, None, &[])
    else {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: Some(target),
                result: DeconstructionCancelResult::StaleOrder,
            },
        );
        return;
    };
    let terminal_outcomes = terminalize_exact_tasks(world, &terminal_requests);
    if terminal_outcomes
        .iter()
        .any(|outcome| outcome.result != ExactTaskTerminalResult::Applied)
    {
        write_cancel_outcome(
            world,
            DeconstructionCancelOutcome {
                order: request.order,
                target: Some(target),
                result: DeconstructionCancelResult::StaleOrder,
            },
        );
        return;
    }
    if let Ok(mut target_entity) = world.get_entity_mut(target)
        && target_entity
            .get::<DeconstructionPending>()
            .is_some_and(|pending| pending.order == request.order)
    {
        target_entity.remove::<DeconstructionPending>();
    }
    for order in orders_to_cancel {
        if let Ok(order_entity) = world.get_entity_mut(order) {
            order_entity.despawn();
        }
        canceled_orders.insert(order, target);
    }
    write_cancel_outcome(
        world,
        DeconstructionCancelOutcome {
            order: request.order,
            target: Some(target),
            result: DeconstructionCancelResult::Canceled,
        },
    );
}

fn validate_commit(
    world: &mut World,
    current_epoch: u64,
    request: DeconstructionCommitRequest,
) -> Result<PreparedCommit, CommitFailure> {
    if request.world_epoch != current_epoch {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleWorld,
        ));
    }
    if request.identity.assignment_entity != request.order
        || request.identity.current_target_entity != request.order
        || request.identity.current_work_type != WorkType::Deconstruct
    {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleIdentity,
        ));
    }
    if world.get::<DeconstructionOrder>(request.order).is_none() {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let identity_matches = world.get::<DamnedSoul>(request.worker).is_some()
        && world.get::<Transform>(request.worker).is_some()
        && world
            .get::<ActiveTaskIdentity>(request.worker)
            .is_some_and(|identity| *identity == request.identity)
        && world
            .get::<WorkingOn>(request.worker)
            .is_some_and(|working| request.identity.matches_working_on(Some(working.0)))
        && world
            .get::<AssignedTask>(request.worker)
            .is_some_and(|task| {
                matches!(
                    task,
                    AssignedTask::Deconstruct(data)
                        if data.order == request.order
                            && data.target == request.target
                            && data.phase == DeconstructPhase::AwaitingCommit
                ) && task.work_type() == Some(request.identity.current_work_type)
            });
    if !identity_matches {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleIdentity,
        ));
    }
    if world
        .get::<Designation>(request.order)
        .is_none_or(|designation| designation.work_type != WorkType::Deconstruct)
    {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let Some(current_target) = world
        .get::<TargetDeconstructionRoot>(request.order)
        .map(|target| target.0)
    else {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    };
    if current_target != request.target {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    if world.get_entity(request.target).is_err() {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    if world
        .get::<DeconstructionPending>(request.target)
        .is_none_or(|pending| pending.order != request.order)
    {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let orders_to_despawn =
        match related_deconstruction_orders_for_target(world, request.target, request.order) {
            Ok(orders) => orders,
            Err(RelatedOrdersFailure::CanonicalInvalid) => {
                return Err(CommitFailure::orphaned_order(
                    DeconstructionCommitResult::StaleTarget,
                ));
            }
            Err(RelatedOrdersFailure::MalformedSibling) => {
                return Err(CommitFailure::blocked(
                    DeconstructionCommitResult::StaleTarget,
                    DeconstructionBlockReason::StaleTarget,
                    TaskDiagnosticDomainMask::TASK,
                ));
            }
        };
    if world
        .get::<DeconstructionCommitClaim>(request.target)
        .is_some_and(|claim| {
            claim.world_epoch != request.world_epoch || claim.order != request.order
        })
    {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::Duplicate,
        ));
    }
    if world.get::<MovePlanned>(request.target).is_some()
        || world.get::<PendingBuildingMove>(request.target).is_some()
        || move_task_references(world, request.target)
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::Moving,
            DeconstructionBlockReason::Moving,
            TaskDiagnosticDomainMask::TASK,
        ));
    }
    if world
        .get::<DeconstructionBlocker>(request.order)
        .is_some_and(|blocker| blocker.active)
    {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::UnsupportedTarget,
        ));
    }

    let Some((kind, is_provisional)) = world
        .get::<Building>(request.target)
        .map(|building| (building.kind, building.is_provisional))
    else {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    };
    let Some(target_position) = world
        .get::<Transform>(request.target)
        .map(|transform| transform.translation.truncate())
    else {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    };
    if is_provisional {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let resolved = resolve_deconstruction_target(world, request.target).map_err(|_| {
        CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        )
    })?;
    if resolved.root != request.target || resolved.class.building_type() != kind {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::OwnerMismatch,
            DeconstructionBlockReason::OwnerMismatch,
            TaskDiagnosticDomainMask::TASK,
        ));
    }
    if !supports_deconstruction_cleanup(kind)
        || !designation_target_shape_is_supported(world, resolved)
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ));
    }

    let (owner_snapshot, anchor) = {
        let Some(map) = world.get_resource::<WorldMap>() else {
            return Err(CommitFailure::blocked(
                DeconstructionCommitResult::OwnerMismatch,
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
            ));
        };
        let owner_snapshot = map.snapshot_owner(request.target);
        let anchor = WorldMap::world_to_grid(target_position);
        let order_grid_matches = world
            .get::<Transform>(request.order)
            .is_some_and(|transform| {
                WorldMap::world_to_grid(transform.translation.truncate()) == anchor
            });
        let owner_is_exact = order_grid_matches
            && target_owner_snapshot_is_exact(map, kind, &owner_snapshot, anchor);
        if !owner_is_exact {
            return Err(CommitFailure::blocked(
                DeconstructionCommitResult::OwnerMismatch,
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
            ));
        }
        (owner_snapshot, anchor)
    };
    let soul_spa_tiles = prepare_soul_spa_tiles(world, request.target, kind, &owner_snapshot)?;
    let power_topology = prepare_power_topology(world, request.target, kind)?;
    let visual_entities = building_visual_entities(world, request.target);
    let recovery = prepare_facility_recovery(
        world,
        request.target,
        kind,
        &owner_snapshot,
        anchor,
        deconstruction_salvage(kind),
    )
    .map_err(recovery_failure)?;
    if !recovery.spawned_items.is_empty()
        && world.get_resource::<ResourceItemVisualHandles>().is_none()
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ));
    }
    let mut removed_owners = recovery.removed_owner_entities(request.target);
    removed_owners.extend(soul_spa_tiles.iter().map(|tile| tile.entity));
    removed_owners.sort_unstable_by_key(|entity| entity.to_bits());
    removed_owners.dedup();
    let transport_requests = transport_requests_referencing_removed_owners(world, &removed_owners);
    if transport_requests
        .iter()
        .any(|request| removed_owners.contains(request))
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ));
    }

    let mut cleanup_references =
        Vec::with_capacity(1 + orders_to_despawn.len() + transport_requests.len());
    cleanup_references.push(request.target);
    cleanup_references.extend(orders_to_despawn.iter().copied());
    cleanup_references.extend(transport_requests.iter().copied());
    cleanup_references.extend(recovery.cleanup_reference_entities());
    cleanup_references.extend(soul_spa_tiles.iter().map(|tile| tile.entity));
    cleanup_references.sort_unstable_by_key(|entity| entity.to_bits());
    cleanup_references.dedup();
    let preserve_loaded_carriers = recovery
        .wheelbarrows
        .iter()
        .map(|carrier| carrier.entity)
        .collect::<Vec<_>>();
    let terminal_requests = prepare_owner_task_terminals(
        world,
        &cleanup_references,
        Some(CompletingExactTask {
            worker: request.worker,
            identity: request.identity,
            expectation: ExactTaskExpectation::DeconstructionAwaitingCommit {
                order: request.order,
                target: request.target,
            },
        }),
        &preserve_loaded_carriers,
    )
    .map_err(|()| {
        CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        )
    })?;

    Ok(PreparedCommit {
        kind,
        owner_snapshot,
        soul_spa_tiles,
        power_topology,
        visual_entities,
        recovery,
        removed_owners,
        terminal_requests,
        transport_requests,
        orders_to_despawn,
    })
}

fn target_owner_snapshot_is_exact(
    map: &WorldMap,
    kind: BuildingType,
    snapshot: &WorldMapOwnerSnapshot,
    anchor: (i32, i32),
) -> bool {
    if !snapshot.stockpile_grids.is_empty() {
        return false;
    }

    let exact_single = |grids: &[(i32, i32)]| grids == [anchor];
    match kind {
        BuildingType::Wall
        | BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::OutdoorLamp => {
            exact_single(&snapshot.building_grids)
                && snapshot.floor_grids.is_empty()
                && snapshot.door_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Door => {
            exact_single(&snapshot.building_grids)
                && exact_single(&snapshot.door_grids)
                && map.door_state(anchor.0, anchor.1).is_some()
                && snapshot.floor_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Floor => {
            exact_single(&snapshot.floor_grids)
                && snapshot.building_grids.is_empty()
                && snapshot.door_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking
        | BuildingType::SoulSpa => {
            exact_rectangle(&snapshot.building_grids, 2, 2)
                && snapshot.building_grids.contains(&anchor)
                && snapshot.floor_grids.is_empty()
                && snapshot.door_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Bridge => {
            exact_rectangle(&snapshot.building_grids, 2, 5)
                && snapshot.building_grids.contains(&anchor)
                && snapshot.bridge_grids == snapshot.building_grids
                && snapshot.floor_grids.is_empty()
                && snapshot.door_grids.is_empty()
        }
    }
}

fn exact_rectangle(grids: &[(i32, i32)], width: i32, height: i32) -> bool {
    if grids.len() != (width * height) as usize {
        return false;
    }
    let Some(min_x) = grids.iter().map(|grid| grid.0).min() else {
        return false;
    };
    let Some(min_y) = grids.iter().map(|grid| grid.1).min() else {
        return false;
    };
    (min_y..min_y + height)
        .flat_map(|y| (min_x..min_x + width).map(move |x| (x, y)))
        .all(|grid| {
            grids
                .binary_search_by_key(&(grid.1, grid.0), |&(x, y)| (y, x))
                .is_ok()
        })
}

fn prepare_soul_spa_tiles(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
    owner_snapshot: &WorldMapOwnerSnapshot,
) -> Result<Vec<SoulSpaTileSnapshot>, CommitFailure> {
    let mut query = world.query::<(
        Entity,
        &SoulSpaTile,
        Option<&ChildOf>,
        Option<&Designation>,
        Option<&hw_jobs::TaskSlots>,
    )>();
    let mut snapshots = Vec::new();
    for (entity, tile, parent, designation, slots) in query.iter(world) {
        let child_of_target = parent.is_some_and(|parent| parent.parent() == target);
        if tile.parent_site != target && !child_of_target {
            continue;
        }
        if kind != BuildingType::SoulSpa
            || tile.parent_site != target
            || parent.is_some_and(|parent| parent.parent() != target)
            || designation
                .is_none_or(|designation| designation.work_type != WorkType::GeneratePower)
            || slots.is_none_or(|slots| slots.max != 1)
        {
            return Err(owner_mismatch_failure());
        }
        snapshots.push(SoulSpaTileSnapshot {
            entity,
            grid: tile.grid_pos,
        });
    }
    snapshots.sort_unstable_by_key(|snapshot| snapshot.entity.to_bits());

    if kind != BuildingType::SoulSpa {
        return snapshots
            .is_empty()
            .then_some(snapshots)
            .ok_or_else(owner_mismatch_failure);
    }
    if world
        .get::<SoulSpaSite>(target)
        .is_none_or(|site| site.phase != SoulSpaPhase::Operational)
        || snapshots.len() != 4
    {
        return Err(owner_mismatch_failure());
    }
    let mut tile_grids = snapshots.iter().map(|tile| tile.grid).collect::<Vec<_>>();
    tile_grids.sort_unstable_by_key(|&(x, y)| (y, x));
    tile_grids.dedup();
    if tile_grids != owner_snapshot.building_grids {
        return Err(owner_mismatch_failure());
    }
    Ok(snapshots)
}

fn prepare_power_topology(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
) -> Result<PowerTopologySnapshot, CommitFailure> {
    let snapshot = PowerTopologySnapshot {
        generator_grid: world.get::<GeneratesFor>(target).map(|relation| relation.0),
        consumer_grid: world.get::<ConsumesFrom>(target).map(|relation| relation.0),
    };
    let marker_shape_matches = match kind {
        BuildingType::SoulSpa => {
            world.get::<PowerGenerator>(target).is_some()
                && world.get::<PowerConsumer>(target).is_none()
                && snapshot.consumer_grid.is_none()
        }
        BuildingType::OutdoorLamp => {
            world.get::<PowerConsumer>(target).is_some()
                && world.get::<PowerGenerator>(target).is_none()
                && snapshot.generator_grid.is_none()
        }
        _ => {
            world.get::<PowerGenerator>(target).is_none()
                && world.get::<PowerConsumer>(target).is_none()
                && snapshot == PowerTopologySnapshot::default()
        }
    };
    if !marker_shape_matches || !power_topology_reverse_edges_match(world, target, snapshot) {
        return Err(owner_mismatch_failure());
    }
    Ok(snapshot)
}

fn power_topology_reverse_edges_match(
    world: &mut World,
    target: Entity,
    snapshot: PowerTopologySnapshot,
) -> bool {
    let mut generator_query = world.query::<(Entity, &GridGenerators)>();
    let mut generator_grids = generator_query
        .iter(world)
        .filter_map(|(grid, generators)| {
            generators
                .iter()
                .any(|source| *source == target)
                .then_some(grid)
        })
        .collect::<Vec<_>>();
    generator_grids.sort_unstable_by_key(|entity| entity.to_bits());
    let mut consumer_query = world.query::<(Entity, &GridConsumers)>();
    let mut consumer_grids = consumer_query
        .iter(world)
        .filter_map(|(grid, consumers)| {
            consumers
                .iter()
                .any(|source| *source == target)
                .then_some(grid)
        })
        .collect::<Vec<_>>();
    consumer_grids.sort_unstable_by_key(|entity| entity.to_bits());

    generator_grids == snapshot.generator_grid.into_iter().collect::<Vec<_>>()
        && consumer_grids == snapshot.consumer_grid.into_iter().collect::<Vec<_>>()
}

fn building_visual_entities(world: &mut World, target: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &Building3dVisual)>();
    let mut visuals = query
        .iter(world)
        .filter_map(|(entity, visual)| (visual.owner == target).then_some(entity))
        .collect::<Vec<_>>();
    visuals.sort_unstable_by_key(|entity| entity.to_bits());
    visuals
}

fn owner_mismatch_failure() -> CommitFailure {
    CommitFailure::blocked(
        DeconstructionCommitResult::OwnerMismatch,
        DeconstructionBlockReason::OwnerMismatch,
        TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
    )
}

fn recovery_failure(failure: RecoveryPlanFailure) -> CommitFailure {
    match failure {
        RecoveryPlanFailure::OwnerMismatch => CommitFailure::blocked(
            DeconstructionCommitResult::OwnerMismatch,
            DeconstructionBlockReason::OwnerMismatch,
            TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
        ),
        RecoveryPlanFailure::NoSafeRecovery => CommitFailure::blocked(
            DeconstructionCommitResult::NoSafeRecovery,
            DeconstructionBlockReason::NoSafeRecovery,
            TaskDiagnosticDomainMask::TOPOLOGY.union(TaskDiagnosticDomainMask::AVAILABILITY),
        ),
        RecoveryPlanFailure::InconsistentMixerInventory => CommitFailure::blocked(
            DeconstructionCommitResult::InconsistentMixerInventory,
            DeconstructionBlockReason::InconsistentMixerInventory,
            TaskDiagnosticDomainMask::AVAILABILITY,
        ),
        RecoveryPlanFailure::UnsupportedTarget => CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ),
    }
}

fn commit_deconstruction(
    world: &mut World,
    request: DeconstructionCommitRequest,
    prepared: PreparedCommit,
) -> Result<SuccessfulCommitMetrics, CommitFailure> {
    world
        .entity_mut(request.target)
        .insert(DeconstructionCommitClaim {
            world_epoch: request.world_epoch,
            order: request.order,
        });

    if !prepared_snapshot_still_matches(world, request.target, &prepared) {
        release_commit_claim(world, request);
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::OwnerMismatch,
            DeconstructionBlockReason::OwnerMismatch,
            TaskDiagnosticDomainMask::TASK
                .union(TaskDiagnosticDomainMask::TOPOLOGY)
                .union(TaskDiagnosticDomainMask::AVAILABILITY),
        ));
    }

    let terminal_outcomes = terminalize_exact_tasks(world, &prepared.terminal_requests);
    if terminal_outcomes
        .iter()
        .any(|outcome| outcome.result != ExactTaskTerminalResult::Applied)
    {
        release_commit_claim(world, request);
        let winner_failed = terminal_outcomes.iter().any(|outcome| {
            outcome.worker == request.worker
                && !matches!(
                    outcome.result,
                    ExactTaskTerminalResult::Applied | ExactTaskTerminalResult::BatchAborted
                )
        });
        return Err(if winner_failed {
            CommitFailure::untouched(DeconstructionCommitResult::StaleIdentity)
        } else {
            CommitFailure::blocked(
                DeconstructionCommitResult::UnsupportedTarget,
                DeconstructionBlockReason::UnsupportedTarget,
                TaskDiagnosticDomainMask::TASK,
            )
        });
    }

    let transport_cleanup = close_transport_requests_for_removed_owners(
        world,
        &prepared.removed_owners,
        &prepared.transport_requests,
    );
    assert_eq!(
        transport_cleanup,
        OwnerTransportCleanupResult::Applied,
        "prevalidated deconstruction transport cleanup changed during exclusive apply"
    );
    if prepared.kind == BuildingType::RestArea {
        let rest_release = release_rest_area_for_removed_owner(
            world,
            request.target,
            &prepared.recovery.rest_sources,
        );
        assert_eq!(
            rest_release,
            RestAreaReleaseResult::Applied,
            "prevalidated RestArea release changed during exclusive apply"
        );
    }
    if let Some(mut cache) = world.get_resource_mut::<SharedResourceCache>() {
        for &owner in &prepared.removed_owners {
            cache.clear_owner_reservations(owner);
        }
    }

    {
        let mut map = world.resource_mut::<WorldMap>();
        match prepared.kind {
            BuildingType::Door => {
                for grid in &prepared.owner_snapshot.door_grids {
                    let cleared = map.clear_door_if_owned(*grid, request.target);
                    debug_assert!(cleared, "validated door owner changed in exclusive commit");
                }
            }
            BuildingType::Floor => {
                for grid in &prepared.owner_snapshot.floor_grids {
                    let cleared = map.clear_floor_if_owned(*grid, request.target);
                    debug_assert!(cleared, "validated floor owner changed in exclusive commit");
                }
            }
            BuildingType::Bridge => {
                for grid in &prepared.owner_snapshot.bridge_grids {
                    let cleared = map.clear_bridge_if_owned(*grid, request.target);
                    debug_assert!(
                        cleared,
                        "validated bridge owner changed in exclusive commit"
                    );
                }
            }
            BuildingType::SoulSpa | BuildingType::OutdoorLamp => {
                for grid in &prepared.owner_snapshot.building_grids {
                    let cleared = map.clear_building_if_owned(*grid, request.target);
                    debug_assert!(
                        cleared,
                        "validated passable building owner changed in exclusive commit"
                    );
                }
            }
            _ => {
                for grid in &prepared.owner_snapshot.building_grids {
                    let cleared = map.clear_building_occupancy_if_owned(*grid, request.target);
                    debug_assert!(
                        cleared,
                        "validated deconstruction owner changed in exclusive commit"
                    );
                }
            }
        }
        for companion in &prepared.recovery.companions_to_remove {
            for grid in &companion.owner_snapshot.stockpile_grids {
                let cleared = map.clear_stockpile_tile_if_owned(*grid, companion.entity);
                debug_assert!(
                    cleared,
                    "validated companion stockpile owner changed in exclusive commit"
                );
            }
        }
    }

    let dirty_grids = prepared
        .owner_snapshot
        .building_grids
        .iter()
        .chain(&prepared.owner_snapshot.floor_grids)
        .copied()
        .collect::<Vec<_>>();
    if let Some(mut room_detection) = world.get_resource_mut::<RoomDetectionState>() {
        room_detection.mark_dirty_many(dirty_grids.iter().copied());
    }
    if matches!(prepared.kind, BuildingType::Wall | BuildingType::Door)
        && let Some(mut wall_connections) =
            world.get_resource_mut::<hw_visual::wall_connection::WallConnectionDirty>()
    {
        wall_connections.mark_removed(dirty_grids.iter().copied());
    }

    #[cfg(feature = "profiling")]
    let recovery_items_spawned = prepared.recovery.spawned_items.len() as u64;
    apply_facility_recovery(world, &prepared.recovery);

    for visual in prepared.visual_entities {
        if let Ok(entity) = world.get_entity_mut(visual) {
            entity.despawn();
        }
    }
    for tile in prepared.soul_spa_tiles {
        if let Ok(entity) = world.get_entity_mut(tile.entity) {
            entity.despawn();
        }
    }
    for order in prepared.orders_to_despawn {
        if let Ok(order) = world.get_entity_mut(order) {
            order.despawn();
        }
    }
    if let Ok(target) = world.get_entity_mut(request.target) {
        target.despawn();
    }
    if matches!(
        prepared.kind,
        BuildingType::SoulSpa | BuildingType::OutdoorLamp
    ) && let Some(mut dirty) = world.get_resource_mut::<EnergyUpdateDirty>()
    {
        dirty.request_full_rebuild();
    }

    Ok(SuccessfulCommitMetrics {
        #[cfg(feature = "profiling")]
        recovery_items_spawned,
    })
}

#[cfg(feature = "profiling")]
fn record_commit_validation_pass(world: &mut World) {
    if let Some(mut metrics) = world.get_resource_mut::<DeconstructionPerfMetrics>() {
        metrics.commit_validation_passes = metrics.commit_validation_passes.saturating_add(1);
    }
}

#[cfg(feature = "profiling")]
fn record_successful_commit_metrics(
    world: &mut World,
    commit_metrics: SuccessfulCommitMetrics,
    elapsed: std::time::Duration,
) {
    if let Some(mut metrics) = world.get_resource_mut::<DeconstructionPerfMetrics>() {
        metrics.successful_cleanup_transactions =
            metrics.successful_cleanup_transactions.saturating_add(1);
        metrics.recovery_items_spawned = metrics
            .recovery_items_spawned
            .saturating_add(commit_metrics.recovery_items_spawned);
        metrics.successful_transaction_elapsed_ns = metrics
            .successful_transaction_elapsed_ns
            .saturating_add(elapsed.as_nanos());
    }
}

fn prepared_snapshot_still_matches(
    world: &mut World,
    target: Entity,
    prepared: &PreparedCommit,
) -> bool {
    let world_map_matches = world.get_resource::<WorldMap>().is_some_and(|map| {
        map.snapshot_owner(target) == prepared.owner_snapshot
            && prepared
                .recovery
                .companions_to_remove
                .iter()
                .all(|companion| map.snapshot_owner(companion.entity) == companion.owner_snapshot)
    });
    if !world_map_matches {
        return false;
    }
    let soul_spa_tiles_match =
        prepare_soul_spa_tiles(world, target, prepared.kind, &prepared.owner_snapshot)
            .is_ok_and(|tiles| tiles == prepared.soul_spa_tiles);
    if !soul_spa_tiles_match {
        return false;
    }
    let power_topology_matches = prepare_power_topology(world, target, prepared.kind)
        .is_ok_and(|topology| topology == prepared.power_topology);
    if !power_topology_matches
        || building_visual_entities(world, target) != prepared.visual_entities
    {
        return false;
    }
    recovery_plan_still_matches(world, &prepared.recovery, target)
        && transport_requests_referencing_removed_owners(world, &prepared.removed_owners)
            == prepared.transport_requests
        && (prepared.kind != BuildingType::RestArea
            || rest_area_relationship_sources(world, target) == prepared.recovery.rest_sources)
}

fn release_commit_claim(world: &mut World, request: DeconstructionCommitRequest) {
    if let Ok(mut target) = world.get_entity_mut(request.target)
        && target
            .get::<DeconstructionCommitClaim>()
            .is_some_and(|claim| {
                claim.world_epoch == request.world_epoch && claim.order == request.order
            })
    {
        target.remove::<DeconstructionCommitClaim>();
    }
}

fn handle_commit_failure(
    world: &mut World,
    request: DeconstructionCommitRequest,
    failure: CommitFailure,
) {
    if failure.result != DeconstructionCommitResult::StaleWorld {
        release_commit_claim(world, request);
    }
    if failure.result == DeconstructionCommitResult::StaleWorld {
        return;
    }
    if failure.result == DeconstructionCommitResult::StaleIdentity {
        terminalize_commit_worker(
            world,
            request,
            ExactTaskTerminalDisposition::Abort {
                emit_abandoned: false,
            },
        );
        return;
    }
    if failure.discard_orphaned_order {
        let terminalized = prepare_owner_task_terminals(world, &[request.order], None, &[])
            .ok()
            .map(|terminal_requests| terminalize_exact_tasks(world, &terminal_requests))
            .is_some_and(|outcomes| {
                outcomes
                    .iter()
                    .all(|outcome| outcome.result == ExactTaskTerminalResult::Applied)
            });
        if terminalized {
            remove_orphaned_pending(world, request.target, request.order);
            if let Ok(order) = world.get_entity_mut(request.order) {
                order.despawn();
            }
        }
        return;
    }
    let terminal_result = terminalize_commit_worker(
        world,
        request,
        ExactTaskTerminalDisposition::Abort {
            emit_abandoned: false,
        },
    );
    if let Some((reason, domains)) = failure.blocker
        && terminal_result == ExactTaskTerminalResult::Applied
    {
        let blocker =
            deconstruction_blocker_after_worker_cleanup(world, request.order, reason, domains);
        if let Ok(mut order) = world.get_entity_mut(request.order) {
            order.insert(blocker);
        }
    }
}

fn remove_orphaned_pending(world: &mut World, target: Entity, discarded_order: Entity) {
    let Some(pending_order) = world
        .get::<DeconstructionPending>(target)
        .map(|pending| pending.order)
    else {
        return;
    };
    let pending_is_live = world.get::<DeconstructionOrder>(pending_order).is_some()
        && world
            .get::<Designation>(pending_order)
            .is_some_and(|designation| designation.work_type == WorkType::Deconstruct)
        && world
            .get::<TargetDeconstructionRoot>(pending_order)
            .is_some_and(|relation| relation.0 == target);
    if (pending_order == discarded_order || !pending_is_live)
        && let Ok(mut target) = world.get_entity_mut(target)
    {
        target.remove::<DeconstructionPending>();
    }
}

fn deconstruction_blocker_after_worker_cleanup(
    world: &World,
    order: Entity,
    reason: DeconstructionBlockReason,
    domains: TaskDiagnosticDomainMask,
) -> DeconstructionBlocker {
    let Some(revisions) = world.get_resource::<TaskDiagnosticInputRevisions>() else {
        return DeconstructionBlocker::pending(reason, domains);
    };
    let mut stamp = revisions.stamp_for(order);
    if domains.contains(TaskDiagnosticDomainMask::TASK) {
        // Removing the exact WorkingOn edge changes TaskWorkers once. Rebase
        // over that known cleanup bump; any additional task-domain change
        // before the next sync still makes this blocker stale.
        stamp.task = stamp.task.wrapping_add(1);
    }
    DeconstructionBlocker::armed(reason, domains, stamp)
}

fn terminalize_commit_worker(
    world: &mut World,
    request: DeconstructionCommitRequest,
    disposition: ExactTaskTerminalDisposition,
) -> ExactTaskTerminalResult {
    terminalize_exact_tasks(
        world,
        &[ExactTaskTerminalRequest {
            worker: request.worker,
            expected_identity: request.identity,
            expectation: ExactTaskExpectation::DeconstructionAwaitingCommit {
                order: request.order,
                target: request.target,
            },
            disposition,
        }],
    )[0]
    .result
}

fn related_deconstruction_orders_for_target(
    world: &World,
    target: Entity,
    canonical_order: Entity,
) -> Result<Vec<Entity>, RelatedOrdersFailure> {
    let relations = world
        .get::<hw_jobs::DeconstructionOrders>(target)
        .ok_or(RelatedOrdersFailure::CanonicalInvalid)?;
    let mut orders = relations.iter().copied().collect::<Vec<_>>();
    orders.sort_unstable_by_key(|entity| entity.to_bits());
    if !orders.contains(&canonical_order) {
        return Err(RelatedOrdersFailure::CanonicalInvalid);
    }
    let canonical_is_valid = world.get::<DeconstructionOrder>(canonical_order).is_some()
        && world
            .get::<Designation>(canonical_order)
            .is_some_and(|designation| designation.work_type == WorkType::Deconstruct)
        && world
            .get::<TargetDeconstructionRoot>(canonical_order)
            .is_some_and(|relation| relation.0 == target);
    if !canonical_is_valid {
        return Err(RelatedOrdersFailure::CanonicalInvalid);
    }
    for &sibling in orders.iter().filter(|&&order| order != canonical_order) {
        let sibling_is_owned_order = world.get::<DeconstructionOrder>(sibling).is_some()
            && world
                .get::<TargetDeconstructionRoot>(sibling)
                .is_some_and(|relation| relation.0 == target);
        if !sibling_is_owned_order {
            return Err(RelatedOrdersFailure::MalformedSibling);
        }
    }
    Ok(orders)
}

fn move_task_references(world: &mut World, target: Entity) -> bool {
    let durable_task_exists = {
        let mut query = world.query::<&MovePlantTask>();
        query.iter(world).any(|task| task.building == target)
    };
    if durable_task_exists {
        return true;
    }
    let mut query = world.query::<&AssignedTask>();
    query
        .iter(world)
        .any(|task| matches!(task, AssignedTask::MovePlant(data) if data.building == target))
}

const fn commit_outcome_base(request: DeconstructionCommitRequest) -> DeconstructionCommitOutcome {
    DeconstructionCommitOutcome {
        worker: request.worker,
        order: request.order,
        target: request.target,
        result: DeconstructionCommitResult::StaleIdentity,
    }
}

fn write_commit_outcome(world: &mut World, outcome: DeconstructionCommitOutcome) {
    world
        .resource_mut::<Messages<DeconstructionCommitOutcome>>()
        .write(outcome);
}

fn write_cancel_outcome(world: &mut World, outcome: DeconstructionCancelOutcome) {
    world
        .resource_mut::<Messages<DeconstructionCancelOutcome>>()
        .write(outcome);
}
