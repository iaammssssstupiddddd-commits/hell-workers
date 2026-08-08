//! Fixed-step performance fixture for the deconstruction owner transaction.

use super::fixture::{PerfSetupFamiliarFilter, PerfSetupSoulFilter, fixture_free_grids};
use super::*;

use crate::systems::jobs::DeconstructionPerfMetrics;
use bevy::ecs::system::SystemParam;
use hw_core::WorldEpoch;
use hw_core::relationships::{CommandedBy, ManagedBy, WorkingOn};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, BonePile, Building, BuildingType, DeconstructData,
    DeconstructPhase, DeconstructionCommitOutcome, DeconstructionCommitRequest,
    DeconstructionOrder, DeconstructionPending, Designation, PlayerIssuedDesignation, Priority,
    TargetDeconstructionRoot, TaskSlots,
};
use hw_logistics::{ResourceItem, ResourceType};

const MEDIUM_COMPLETED_BUILDINGS: usize = 100;
const RECOVERY_BONE_COUNT: usize = 5;

/// Runtime ledger for the deterministic deconstruction fixture.
///
/// Entity handles are intentionally kept out of audit checksums. They are only
/// used by this one-run driver to correlate the request, outcome, and cleanup.
#[derive(Resource, Default)]
pub(crate) struct DeconstructionPerfFixtureState {
    pub(super) setup_complete: bool,
    pub(super) request_issued: bool,
    pub(super) commit_result_seen: bool,
    pub(super) committed: bool,
    pub(super) target: Option<Entity>,
    pub(super) order: Option<Entity>,
    pub(super) worker: Option<Entity>,
    pub(super) identity: Option<ActiveTaskIdentity>,
    pub(super) initial_bone_items: Option<usize>,
    pub(super) metrics_before_request: Option<DeconstructionPerfMetrics>,
    pub(super) metrics_after_commit: Option<DeconstructionPerfMetrics>,
    pub(super) initial_completed_buildings: usize,
    pub(super) final_completed_buildings: usize,
    pub(super) building_type_count: usize,
    pub(super) recovery_items: usize,
    pub(super) commit_requests: usize,
    pub(super) committed_count: usize,
    pub(super) commit_validation_passes: u64,
    pub(super) successful_cleanup_transactions: u64,
    pub(super) recovery_items_spawned: u64,
    pub(super) post_commit_updates: usize,
    pub(super) steady_state_validation_delta: u64,
    pub(super) successful_transaction_elapsed_ns: u128,
}

impl DeconstructionPerfFixtureState {
    pub(super) fn sidecar_csv(&self) -> Result<String, &'static str> {
        if !self.setup_complete {
            return Err("fixture setup did not complete");
        }
        if !self.committed || !self.commit_result_seen {
            return Err("fixture did not observe a committed deconstruction");
        }
        if self.initial_completed_buildings != MEDIUM_COMPLETED_BUILDINGS
            || self.final_completed_buildings + 1 != self.initial_completed_buildings
            || self.building_type_count != BuildingType::ALL.len()
            || self.commit_requests != 1
            || self.committed_count != 1
            || self.recovery_items != RECOVERY_BONE_COUNT
            || self.commit_validation_passes != 1
            || self.successful_cleanup_transactions != 1
            || self.recovery_items_spawned != RECOVERY_BONE_COUNT as u64
            || self.post_commit_updates == 0
            || self.steady_state_validation_delta != 0
            || self.successful_transaction_elapsed_ns == 0
        {
            return Err("fixture transaction contract was not satisfied");
        }

        Ok(format!(
            concat!(
                "schema_version,initial_completed_buildings,final_completed_buildings,",
                "building_type_count,commit_requests,committed,recovery_items,",
                "commit_validation_passes,successful_cleanup_transactions,",
                "recovery_items_spawned,post_commit_updates,",
                "steady_state_validation_delta,successful_transaction_elapsed_ns\n",
                "2,{},{},{},{},{},{},{},{},{},{},{},{}\n"
            ),
            self.initial_completed_buildings,
            self.final_completed_buildings,
            self.building_type_count,
            self.commit_requests,
            self.committed_count,
            self.recovery_items,
            self.commit_validation_passes,
            self.successful_cleanup_transactions,
            self.recovery_items_spawned,
            self.post_commit_updates,
            self.steady_state_validation_delta,
            self.successful_transaction_elapsed_ns,
        ))
    }
}

pub(super) fn configure_deconstruction_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
    q_souls: &mut Query<
        (
            Entity,
            &mut Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
        ),
        PerfSetupSoulFilter,
    >,
    world_map: &mut WorldMapWrite,
    size: PerfScenarioSize,
    state: &mut DeconstructionPerfFixtureState,
) -> bool {
    if size != PerfScenarioSize::Medium {
        error!("PERF_CAPTURE: deconstruction fixture requires medium size");
        return false;
    }

    let mut familiar_entities = Vec::new();
    for (entity, _, mut command, mut operation, _) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
        familiar_entities.push(entity);
    }
    familiar_entities.sort_unstable_by_key(|entity| entity.to_bits());
    let Some(owner) = familiar_entities.first().copied() else {
        error!("PERF_CAPTURE: deconstruction fixture has no familiar owner");
        return false;
    };

    let mut soul_entities = q_souls
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<Vec<_>>();
    soul_entities.sort_unstable_by_key(|entity| entity.to_bits());
    let Some(worker) = soul_entities.first().copied() else {
        error!("PERF_CAPTURE: deconstruction fixture has no soul worker");
        return false;
    };

    // The audit isolates one owner transaction. Keep every background Soul in
    // the existing commanded-idle path so its autonomous idle/pathfinding
    // decisions cannot race the fixed-step checkpoint boundary. The selected
    // worker remains uncommanded because the fixture drives its exact
    // deconstruction identity directly below.
    for soul in soul_entities.iter().copied().filter(|soul| *soul != worker) {
        commands.entity(soul).insert(CommandedBy(owner));
    }

    let mut grids = fixture_free_grids(world_map.as_ref(), MEDIUM_COMPLETED_BUILDINGS);
    if grids.len() != MEDIUM_COMPLETED_BUILDINGS {
        error!(
            "PERF_CAPTURE: deconstruction fixture found only {} of {} free grids",
            grids.len(),
            MEDIUM_COMPLETED_BUILDINGS
        );
        return false;
    }
    grids.sort_unstable();

    let target_ordinal = BuildingType::ALL
        .iter()
        .position(|kind| *kind == BuildingType::BonePile)
        .expect("BonePile must remain in BuildingType::ALL");
    let mut target = None;
    let mut target_grid = None;
    for (ordinal, grid) in grids
        .iter()
        .copied()
        .take(MEDIUM_COMPLETED_BUILDINGS)
        .enumerate()
    {
        let kind = BuildingType::ALL[ordinal % BuildingType::ALL.len()];
        let entity = commands
            .spawn((
                Name::new("PerfDeconstructionCompletedBuilding"),
                DeconstructionPerfBuilding,
                Building {
                    kind,
                    is_provisional: false,
                },
                Visibility::Hidden,
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(Z_MAP)),
            ))
            .id();
        if kind == BuildingType::BonePile {
            commands.entity(entity).insert(BonePile);
        }
        if ordinal == target_ordinal {
            target = Some(entity);
            target_grid = Some(grid);
            commands.entity(entity).insert(Visibility::Visible);
        }
    }
    let (Some(target), Some(target_grid)) = (target, target_grid) else {
        error!("PERF_CAPTURE: deconstruction fixture failed to create BonePile target");
        return false;
    };

    let target_position = WorldMap::grid_to_world(target_grid.0, target_grid.1);
    let order = commands
        .spawn((
            Name::new("PerfDeconstructionOrder"),
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            PlayerIssuedDesignation,
            Priority(0),
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            ManagedBy(owner),
            Transform::from_translation(target_position.extend(Z_MAP)),
        ))
        .id();
    commands
        .entity(target)
        .insert(DeconstructionPending { order });
    // Only the target is registered in WorldMap. The other 99 completed
    // buildings are stable ECS population for the audit and cannot interfere
    // with path/room topology or the target's exact-owner snapshot.
    world_map.set_building_occupancy(target_grid, target);

    if let Ok((_, mut transform, mut destination, mut path, mut task)) = q_souls.get_mut(worker) {
        transform.translation = target_position.extend(transform.translation.z);
        destination.0 = target_position;
        path.waypoints.clear();
        path.current_index = 0;
        path.planned_destination = None;
        *task = AssignedTask::Deconstruct(DeconstructData {
            order,
            target,
            phase: DeconstructPhase::AwaitingCommit,
        });
    } else {
        error!("PERF_CAPTURE: deconstruction fixture worker disappeared during setup");
        return false;
    }
    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    commands.entity(worker).insert((identity, WorkingOn(order)));

    *state = DeconstructionPerfFixtureState {
        setup_complete: true,
        target: Some(target),
        order: Some(order),
        worker: Some(worker),
        identity: Some(identity),
        initial_completed_buildings: MEDIUM_COMPLETED_BUILDINGS,
        building_type_count: BuildingType::ALL.len(),
        ..default()
    };
    true
}

#[derive(SystemParam)]
pub(crate) struct DeconstructionPerfDriverParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    state: ResMut<'w, DeconstructionPerfFixtureState>,
    epoch: Res<'w, WorldEpoch>,
    metrics: Res<'w, DeconstructionPerfMetrics>,
    commits: MessageWriter<'w, DeconstructionCommitRequest>,
    outcomes: MessageReader<'w, 's, DeconstructionCommitOutcome>,
    q_targets: Query<'w, 's, (), With<DeconstructionPerfBuilding>>,
    q_items: Query<'w, 's, &'static ResourceItem>,
}

pub(crate) fn drive_deconstruction_perf_workload_system(params: DeconstructionPerfDriverParams) {
    let DeconstructionPerfDriverParams {
        config,
        applied,
        mut state,
        epoch,
        metrics,
        mut commits,
        mut outcomes,
        q_targets,
        q_items,
    } = params;
    if !applied.complete() || !config.enabled() || config.workload != PerfWorkload::Deconstruction {
        return;
    }

    for outcome in outcomes.read() {
        if state.order == Some(outcome.order)
            && state.target == Some(outcome.target)
            && outcome.result == hw_jobs::DeconstructionCommitResult::Committed
        {
            state.commit_result_seen = true;
        }
    }

    if !state.request_issued {
        let (Some(worker), Some(order), Some(target), Some(identity)) =
            (state.worker, state.order, state.target, state.identity)
        else {
            return;
        };
        state.initial_bone_items = Some(
            q_items
                .iter()
                .filter(|item| item.0 == ResourceType::Bone)
                .count(),
        );
        commits.write(DeconstructionCommitRequest {
            world_epoch: epoch.get(),
            worker,
            identity,
            order,
            target,
        });
        state.request_issued = true;
        state.commit_requests = 1;
        state.metrics_before_request = Some(*metrics);
        return;
    }

    let target_removed = state
        .target
        .is_some_and(|target| q_targets.get(target).is_err());
    if !state.committed && state.commit_result_seen && target_removed {
        state.committed = true;
        state.committed_count = 1;
        state.final_completed_buildings = q_targets.iter().count();
        let initial_bones = state.initial_bone_items.unwrap_or_default();
        let current_bones = q_items
            .iter()
            .filter(|item| item.0 == ResourceType::Bone)
            .count();
        state.recovery_items = current_bones.saturating_sub(initial_bones);
        let metrics_before_request = state.metrics_before_request.unwrap_or_default();
        state.commit_validation_passes = metrics
            .commit_validation_passes
            .saturating_sub(metrics_before_request.commit_validation_passes);
        state.successful_cleanup_transactions = metrics
            .successful_cleanup_transactions
            .saturating_sub(metrics_before_request.successful_cleanup_transactions);
        state.recovery_items_spawned = metrics
            .recovery_items_spawned
            .saturating_sub(metrics_before_request.recovery_items_spawned);
        state.successful_transaction_elapsed_ns = metrics
            .successful_transaction_elapsed_ns
            .saturating_sub(metrics_before_request.successful_transaction_elapsed_ns);
        state.metrics_after_commit = Some(*metrics);
    }
    if state.committed {
        state.post_commit_updates = state.post_commit_updates.saturating_add(1);
        if let Some(metrics_after_commit) = state.metrics_after_commit {
            state.steady_state_validation_delta = metrics
                .commit_validation_passes
                .saturating_sub(metrics_after_commit.commit_validation_passes);
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct DeconstructionPerfBuilding;
