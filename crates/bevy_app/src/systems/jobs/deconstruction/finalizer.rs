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
    CompletingExactTask, ExactTaskExpectation, ExactTaskTerminalDisposition,
    ExactTaskTerminalRequest, ExactTaskTerminalResult, RestAreaReleaseResult,
    prepare_owner_task_terminals, release_rest_area_for_removed_owner,
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

mod commit;
mod failure;
mod outcome;
mod preflight;
mod protocol;

use outcome::{commit_outcome_base, write_cancel_outcome, write_commit_outcome};

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
    let (cancels, commits) = protocol::drain_sorted_requests(world);

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
            failure::terminalize_commit_worker(
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
            failure::terminalize_commit_worker(
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
        commit::record_commit_validation_pass(world);
        match preflight::validate_commit(world, current_epoch, request) {
            Ok(prepared) => match commit::commit_deconstruction(world, request, prepared) {
                Ok(_commit_metrics) => {
                    committed_targets.insert(request.target);
                    #[cfg(feature = "profiling")]
                    commit::record_successful_commit_metrics(
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
                    failure::handle_commit_failure(world, request, failure);
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
                failure::handle_commit_failure(world, request, failure);
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
        failure::related_deconstruction_orders_for_target(world, target, request.order)
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
