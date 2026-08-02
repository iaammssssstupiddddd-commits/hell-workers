//! Task dashboard snapshot adapter.

use super::actions::{TaskCapabilityRefs, resolve_task_action_capabilities};
use super::dirty::TaskListDirty;
use super::presenter;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::wall_construction::WallTileBlueprint;
use crate::systems::jobs::{
    Blueprint, BonePile, Designation, PlayerIssuedDesignation, Priority, Rock, SandPile, Tree,
    WorkType,
};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::{
    ManualTransportRequest, TransportRequest, TransportRequestFixedSource,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::{ManagedBy, TaskWorkers};
use hw_familiar_ai::{AutoGatherDesignation, FamiliarTaskCandidateDiagnostics};
use hw_jobs::{
    TaskDiagnosticClass, TaskDiagnosticCounters, TaskDiagnosticCycleHeader,
    TaskDiagnosticInputRevisions, TaskDiagnosticProducer, TaskDiagnosticProducerMask,
    TaskDiagnosticRecord,
};
use hw_soul_ai::BlueprintAutoBuildDiagnostics;
use hw_ui::panels::task_list::{TaskBlockerReason, TaskPriorityTier, TaskStatusSummary};

pub use hw_ui::panels::task_list::TaskEntry;

#[derive(Resource, Default)]
pub struct TaskListState {
    pub snapshot: Vec<TaskEntry>,
    pub summary_total: usize,
    pub summary_high: usize,
    initialized: bool,
}

/// Production Task Dashboard 経路の期間累積 work counter。
#[cfg(feature = "profiling")]
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDashboardPerfMetrics {
    pub state_rebuilds: u32,
    pub snapshot_rows_scanned: u32,
    pub summary_rows_scanned: u32,
    pub snapshot_changes: u32,
    pub summary_changes: u32,
    pub render_rebuilds: u32,
    pub render_input_rows: u32,
    pub render_visible_rows: u32,
    pub render_group_headers: u32,
    pub despawn_roots_requested: u32,
}

/// Task Dashboard update system 全体の実CPU経過時間。
///
/// work counter と異なり決定的ではないため fixed-step checksum には含めず、
/// realtime capture の measure 区間だけを別artifactへ出力する。
#[cfg(feature = "profiling")]
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDashboardTimingMetrics {
    pub active: bool,
    pub system_invocations: u64,
    pub total_elapsed_ns: u64,
}

struct TaskStatusEvidence<'a> {
    familiar_header: Option<&'a TaskDiagnosticCycleHeader>,
    familiar_record: Option<&'a TaskDiagnosticRecord>,
    auto_build_header: Option<&'a TaskDiagnosticCycleHeader>,
    auto_build_record: Option<&'a TaskDiagnosticRecord>,
    revisions: &'a TaskDiagnosticInputRevisions,
}

#[derive(Debug, Clone, Copy)]
struct CompleteProducerEvidence {
    counters: TaskDiagnosticCounters,
    policy_only: bool,
}

type DesignationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static Priority>,
        Option<&'static TaskWorkers>,
        Option<&'static Blueprint>,
        Option<&'static ManagedBy>,
        Option<&'static TransportRequest>,
        Option<&'static ResourceItem>,
        Option<&'static Tree>,
        Option<&'static Rock>,
        Option<&'static SandPile>,
        Option<&'static BonePile>,
    ),
>;

type TaskCapabilityQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Designation,
        Option<&'static Priority>,
        Option<&'static PlayerIssuedDesignation>,
        Option<&'static AutoGatherDesignation>,
        Option<&'static Tree>,
        Option<&'static Rock>,
        Option<&'static Blueprint>,
        Option<&'static ManualTransportRequest>,
        Option<&'static TransportRequestFixedSource>,
        Option<&'static FloorTileBlueprint>,
        Option<&'static WallTileBlueprint>,
        Option<&'static TransportRequest>,
    ),
>;

#[derive(SystemParam)]
pub struct TaskListStateUpdateContext<'w> {
    familiar_diagnostics: Res<'w, FamiliarTaskCandidateDiagnostics>,
    auto_build_diagnostics: Res<'w, BlueprintAutoBuildDiagnostics>,
    revisions: Res<'w, TaskDiagnosticInputRevisions>,
    dirty: ResMut<'w, TaskListDirty>,
    state: ResMut<'w, TaskListState>,
    #[cfg(feature = "profiling")]
    perf_metrics: Option<ResMut<'w, TaskDashboardPerfMetrics>>,
}

pub fn build_task_list_snapshot(
    designations: &DesignationQuery,
    capabilities: &TaskCapabilityQuery,
    familiar_diagnostics: &FamiliarTaskCandidateDiagnostics,
    auto_build_diagnostics: &BlueprintAutoBuildDiagnostics,
    revisions: &TaskDiagnosticInputRevisions,
) -> Vec<TaskEntry> {
    let mut entries = Vec::new();

    for (
        entity,
        _transform,
        designation,
        priority,
        workers,
        blueprint,
        managed_by,
        transport_req,
        resource_item,
        tree,
        rock,
        sand_pile,
        bone_pile,
    ) in designations.iter()
    {
        let work_type = designation.work_type;
        let worker_count = workers.map_or(0, |workers| workers.iter().count());
        let description = presenter::generate_task_description(
            work_type,
            entity,
            presenter::TaskComponentRefs {
                blueprint,
                transport_req,
                resource_item,
                tree,
                rock,
                _sand_pile: sand_pile,
                bone_pile,
            },
        );
        let status = derive_task_status(
            entity,
            work_type,
            worker_count,
            blueprint.is_some() && managed_by.is_none(),
            TaskStatusEvidence {
                familiar_header: familiar_diagnostics.header(),
                familiar_record: familiar_diagnostics.record(entity),
                auto_build_header: auto_build_diagnostics.header(),
                auto_build_record: auto_build_diagnostics.record(entity),
                revisions,
            },
        );

        entries.push(TaskEntry {
            entity,
            work_type,
            description,
            priority: priority.map_or(0, |priority| priority.0),
            worker_count,
            status,
            actions: capabilities.get(entity).map_or(
                hw_ui::panels::task_list::TaskActionCapabilities::READ_ONLY,
                |(
                    designation,
                    priority,
                    player_issued,
                    auto_gather,
                    tree,
                    rock,
                    blueprint,
                    manual_transport,
                    fixed_source,
                    floor_tile,
                    wall_tile,
                    transport_request,
                )| {
                    resolve_task_action_capabilities(TaskCapabilityRefs {
                        designation,
                        has_priority: priority.is_some(),
                        player_issued,
                        auto_gather,
                        tree,
                        rock,
                        blueprint,
                        manual_transport,
                        fixed_source,
                        floor_tile,
                        wall_tile,
                        transport_request,
                    })
                },
            ),
        });
    }

    entries.sort_unstable_by_key(|entry| {
        (
            entry.entity.index_u32(),
            entry.entity.generation().to_bits(),
        )
    });
    entries
}

fn derive_task_status(
    entity: Entity,
    work_type: WorkType,
    worker_count: usize,
    auto_build_applicable: bool,
    evidence: TaskStatusEvidence<'_>,
) -> TaskStatusSummary {
    if worker_count > 0 {
        return TaskStatusSummary::Working;
    }

    let producers = TaskDiagnosticProducerMask::for_task(work_type, auto_build_applicable);
    let Some(familiar) = producer_evidence(
        entity,
        TaskDiagnosticProducer::FamiliarDelegation,
        evidence.familiar_header,
        evidence.familiar_record,
        evidence.revisions,
    ) else {
        return TaskStatusSummary::PendingEvaluation;
    };

    let auto_build = if producers.contains(TaskDiagnosticProducer::BlueprintAutoBuild) {
        let Some(auto_build) = producer_evidence(
            entity,
            TaskDiagnosticProducer::BlueprintAutoBuild,
            evidence.auto_build_header,
            evidence.auto_build_record,
            evidence.revisions,
        ) else {
            return TaskStatusSummary::PendingEvaluation;
        };
        Some(auto_build)
    } else {
        None
    };

    if auto_build.is_none() && familiar.policy_only {
        return TaskStatusSummary::Blocked(TaskBlockerReason::PolicyDisabled);
    }

    let mut counters = familiar.counters;
    if familiar.policy_only {
        counters.clear(TaskDiagnosticClass::PolicyDisabled);
    }
    if let Some(auto_build) = auto_build {
        counters.merge(&auto_build.counters);
    }

    counters.representative().map(map_blocker_reason).map_or(
        TaskStatusSummary::PendingEvaluation,
        TaskStatusSummary::Blocked,
    )
}

fn producer_evidence(
    entity: Entity,
    producer: TaskDiagnosticProducer,
    header: Option<&TaskDiagnosticCycleHeader>,
    record: Option<&TaskDiagnosticRecord>,
    revisions: &TaskDiagnosticInputRevisions,
) -> Option<CompleteProducerEvidence> {
    let header = header?;
    if header.producer != producer
        || header.completed_evaluators != header.eligible_evaluators
        || header.stamp.roster != revisions.roster
    {
        return None;
    }
    if header.eligible_evaluators == 0 {
        let mut counters = TaskDiagnosticCounters::default();
        counters.increment(TaskDiagnosticClass::NoEligibleFamiliar);
        return Some(CompleteProducerEvidence {
            counters,
            policy_only: false,
        });
    }

    let record = record?;
    if record.producer != producer
        || !revisions.is_current(entity, record.stamp, record.domains)
        || record.coverage.submitted_count > 0
        || !record.coverage.is_complete_rejection()
    {
        return None;
    }
    let policy_only = producer == TaskDiagnosticProducer::FamiliarDelegation
        && record.coverage.terminal_votes > 0
        && record.counters.count(TaskDiagnosticClass::PolicyDisabled)
            == record.coverage.terminal_votes;
    let mut counters = record.counters;
    if !policy_only {
        counters.clear(TaskDiagnosticClass::PolicyDisabled);
    }
    Some(CompleteProducerEvidence {
        counters,
        policy_only,
    })
}

fn map_blocker_reason(class: TaskDiagnosticClass) -> TaskBlockerReason {
    match class {
        TaskDiagnosticClass::NoEligibleFamiliar => TaskBlockerReason::NoEligibleFamiliar,
        TaskDiagnosticClass::MissingResourceOrSource => TaskBlockerReason::MissingResourceOrSource,
        TaskDiagnosticClass::Unreachable => TaskBlockerReason::Unreachable,
        TaskDiagnosticClass::TemporaryContention => TaskBlockerReason::TemporaryContention,
        TaskDiagnosticClass::DependencyWaiting => TaskBlockerReason::DependencyWaiting,
        TaskDiagnosticClass::PolicyDisabled => TaskBlockerReason::PolicyDisabled,
    }
}

pub fn build_task_summary(designations: &DesignationQuery) -> (usize, usize) {
    let mut total = 0usize;
    let mut high = 0usize;

    for item in designations.iter() {
        let priority = item.3;
        total += 1;
        if priority.is_some_and(|priority| {
            TaskPriorityTier::from_priority(priority.0) != TaskPriorityTier::Normal
        }) {
            high += 1;
        }
    }

    (total, high)
}

pub fn update_task_list_state_system(
    designations: DesignationQuery,
    capabilities: TaskCapabilityQuery,
    mut context: TaskListStateUpdateContext,
) {
    if context.state.initialized && !context.dirty.state_dirty() {
        return;
    }

    let snapshot = build_task_list_snapshot(
        &designations,
        &capabilities,
        &context.familiar_diagnostics,
        &context.auto_build_diagnostics,
        &context.revisions,
    );
    let (summary_total, summary_high) = build_task_summary(&designations);
    let list_changed = !context.state.initialized || snapshot != context.state.snapshot;
    let summary_changed = !context.state.initialized
        || summary_total != context.state.summary_total
        || summary_high != context.state.summary_high;

    #[cfg(feature = "profiling")]
    if let Some(perf_metrics) = context.perf_metrics.as_deref_mut() {
        perf_metrics.state_rebuilds = perf_metrics.state_rebuilds.saturating_add(1);
        perf_metrics.snapshot_rows_scanned = perf_metrics
            .snapshot_rows_scanned
            .saturating_add(u32::try_from(snapshot.len()).unwrap_or(u32::MAX));
        perf_metrics.summary_rows_scanned = perf_metrics
            .summary_rows_scanned
            .saturating_add(u32::try_from(summary_total).unwrap_or(u32::MAX));
        if list_changed {
            perf_metrics.snapshot_changes = perf_metrics.snapshot_changes.saturating_add(1);
        }
        if summary_changed {
            perf_metrics.summary_changes = perf_metrics.summary_changes.saturating_add(1);
        }
    }

    context.state.snapshot = snapshot;
    context.state.summary_total = summary_total;
    context.state.summary_high = summary_high;
    let was_initialized = context.state.initialized;
    context.state.initialized = true;
    context.dirty.clear_state();

    if !was_initialized || list_changed {
        context.dirty.mark_list();
    } else {
        context.dirty.clear_list();
    }
    if !was_initialized || summary_changed {
        context.dirty.mark_summary();
    } else {
        context.dirty.clear_summary();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::{
        TaskDiagnosticCounters, TaskDiagnosticCoverage, TaskDiagnosticDomainMask,
        TaskDiagnosticInputStamp,
    };

    fn entity() -> Entity {
        Entity::from_raw_u32(1).expect("valid test entity")
    }

    fn header(eligible: u16) -> TaskDiagnosticCycleHeader {
        producer_header(TaskDiagnosticProducer::FamiliarDelegation, eligible)
    }

    fn producer_header(
        producer: TaskDiagnosticProducer,
        eligible: u16,
    ) -> TaskDiagnosticCycleHeader {
        TaskDiagnosticCycleHeader {
            producer,
            cycle: 1,
            eligible_evaluators: eligible,
            completed_evaluators: eligible,
            stamp: TaskDiagnosticInputStamp::default(),
        }
    }

    fn record(class: TaskDiagnosticClass, submitted_count: u16) -> TaskDiagnosticRecord {
        producer_record(
            TaskDiagnosticProducer::FamiliarDelegation,
            &[class],
            submitted_count,
            false,
        )
    }

    fn producer_record(
        producer: TaskDiagnosticProducer,
        classes: &[TaskDiagnosticClass],
        submitted_count: u16,
        partial: bool,
    ) -> TaskDiagnosticRecord {
        let mut counters = TaskDiagnosticCounters::default();
        for class in classes {
            counters.increment(*class);
        }
        let terminal_votes = if submitted_count == 0 && !partial {
            classes.len() as u16
        } else {
            0
        };
        TaskDiagnosticRecord {
            producer,
            coverage: TaskDiagnosticCoverage {
                applicable_evaluators: classes.len().max(1) as u16,
                evaluated_evaluators: classes.len().max(1) as u16,
                terminal_votes,
                submitted_count,
                partial,
            },
            counters,
            stamp: TaskDiagnosticInputStamp::default(),
            domains: TaskDiagnosticDomainMask::ALL,
        }
    }

    fn familiar_evidence<'a>(
        header: Option<&'a TaskDiagnosticCycleHeader>,
        record: Option<&'a TaskDiagnosticRecord>,
        revisions: &'a TaskDiagnosticInputRevisions,
    ) -> TaskStatusEvidence<'a> {
        TaskStatusEvidence {
            familiar_header: header,
            familiar_record: record,
            auto_build_header: None,
            auto_build_record: None,
            revisions,
        }
    }

    #[derive(Resource, Default)]
    struct SummaryReceipt((usize, usize));

    fn capture_task_summary(designations: DesignationQuery, mut receipt: ResMut<SummaryReceipt>) {
        receipt.0 = build_task_summary(&designations);
    }

    #[test]
    fn task_dashboard_summary_uses_the_shared_priority_tiers() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SummaryReceipt>()
            .add_systems(Update, capture_task_summary);
        for priority in [None, Some(0), Some(5), Some(10)] {
            let mut entity = app.world_mut().spawn((
                Transform::default(),
                Designation {
                    work_type: WorkType::Chop,
                },
            ));
            if let Some(priority) = priority {
                entity.insert(Priority(priority));
            }
        }

        app.update();

        assert_eq!(app.world().resource::<SummaryReceipt>().0, (4, 2));
    }

    #[test]
    fn task_dashboard_rebuilds_from_the_first_post_load_producer_cycle() {
        use hw_familiar_ai::familiar_ai::decide::task_management::FamiliarTaskDiagnosticCycle;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<FamiliarTaskCandidateDiagnostics>()
            .init_resource::<BlueprintAutoBuildDiagnostics>()
            .init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskListDirty>()
            .init_resource::<TaskListState>()
            .add_systems(Update, update_task_list_state_system);
        let old_task = app
            .world_mut()
            .spawn((
                Transform::default(),
                Designation {
                    work_type: WorkType::Chop,
                },
                Tree,
            ))
            .id();
        let revisions = TaskDiagnosticInputRevisions::default();
        app.world_mut()
            .resource_mut::<FamiliarTaskCandidateDiagnostics>()
            .publish(FamiliarTaskDiagnosticCycle::new(1, &revisions));

        app.update();
        assert_eq!(app.world().resource::<TaskListState>().snapshot.len(), 1);
        assert_eq!(
            app.world().resource::<TaskListState>().snapshot[0].entity,
            old_task
        );

        app.world_mut().despawn(old_task);
        app.world_mut()
            .insert_resource(FamiliarTaskCandidateDiagnostics::default());
        app.world_mut().insert_resource(TaskListState::default());
        app.world_mut().resource_mut::<TaskListDirty>().mark_all();
        let new_task = app
            .world_mut()
            .spawn((
                Transform::from_xyz(64.0, 32.0, 0.0),
                Designation {
                    work_type: WorkType::Mine,
                },
                Rock,
            ))
            .id();
        app.world_mut()
            .resource_mut::<FamiliarTaskCandidateDiagnostics>()
            .publish(FamiliarTaskDiagnosticCycle::new(1, &revisions));

        app.update();

        let state = app.world().resource::<TaskListState>();
        assert_eq!(state.snapshot.len(), 1);
        assert_eq!(state.snapshot[0].entity, new_task);
        assert_eq!(
            state.snapshot[0].status,
            TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar)
        );
    }

    #[test]
    fn workers_override_stale_or_blocked_diagnostics() {
        let revisions = TaskDiagnosticInputRevisions::default();
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                1,
                false,
                familiar_evidence(
                    Some(&header(1)),
                    Some(&record(TaskDiagnosticClass::Unreachable, 0)),
                    &revisions,
                ),
            ),
            TaskStatusSummary::Working
        );
    }

    #[test]
    fn submitted_without_current_workers_remains_pending() {
        let revisions = TaskDiagnosticInputRevisions::default();
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(
                    Some(&header(1)),
                    Some(&record(TaskDiagnosticClass::Unreachable, 1)),
                    &revisions,
                ),
            ),
            TaskStatusSummary::PendingEvaluation
        );
    }

    #[test]
    fn complete_terminal_rejection_is_blocked() {
        let revisions = TaskDiagnosticInputRevisions::default();
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(
                    Some(&header(1)),
                    Some(&record(TaskDiagnosticClass::Unreachable, 0)),
                    &revisions,
                ),
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::Unreachable)
        );
    }

    #[test]
    fn terminal_diagnostic_classes_map_to_dashboard_blockers() {
        let revisions = TaskDiagnosticInputRevisions::default();
        let cases = [
            (
                TaskDiagnosticClass::NoEligibleFamiliar,
                TaskBlockerReason::NoEligibleFamiliar,
            ),
            (
                TaskDiagnosticClass::MissingResourceOrSource,
                TaskBlockerReason::MissingResourceOrSource,
            ),
            (
                TaskDiagnosticClass::Unreachable,
                TaskBlockerReason::Unreachable,
            ),
            (
                TaskDiagnosticClass::TemporaryContention,
                TaskBlockerReason::TemporaryContention,
            ),
            (
                TaskDiagnosticClass::DependencyWaiting,
                TaskBlockerReason::DependencyWaiting,
            ),
            (
                TaskDiagnosticClass::PolicyDisabled,
                TaskBlockerReason::PolicyDisabled,
            ),
        ];

        for (diagnostic, blocker) in cases {
            assert_eq!(
                derive_task_status(
                    entity(),
                    WorkType::Chop,
                    0,
                    false,
                    familiar_evidence(Some(&header(1)), Some(&record(diagnostic, 0)), &revisions),
                ),
                TaskStatusSummary::Blocked(blocker)
            );
        }
    }

    #[test]
    fn build_without_auto_build_snapshot_is_pending() {
        let revisions = TaskDiagnosticInputRevisions::default();
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Build,
                0,
                true,
                familiar_evidence(Some(&header(0)), None, &revisions),
            ),
            TaskStatusSummary::PendingEvaluation
        );
    }

    #[test]
    fn managed_build_does_not_require_auto_build_evidence() {
        let revisions = TaskDiagnosticInputRevisions::default();
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Build,
                0,
                false,
                familiar_evidence(Some(&header(0)), None, &revisions),
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar)
        );
    }

    #[test]
    fn familiar_policy_is_blocking_only_when_every_terminal_vote_is_policy() {
        let revisions = TaskDiagnosticInputRevisions::default();
        let policy_only = producer_record(
            TaskDiagnosticProducer::FamiliarDelegation,
            &[TaskDiagnosticClass::PolicyDisabled],
            0,
            false,
        );
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(Some(&header(1)), Some(&policy_only), &revisions),
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::PolicyDisabled)
        );

        let mixed = producer_record(
            TaskDiagnosticProducer::FamiliarDelegation,
            &[
                TaskDiagnosticClass::PolicyDisabled,
                TaskDiagnosticClass::Unreachable,
            ],
            0,
            false,
        );
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(Some(&header(2)), Some(&mixed), &revisions),
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::Unreachable)
        );
    }

    #[test]
    fn unowned_build_never_uses_familiar_policy_as_its_only_final_reason() {
        let revisions = TaskDiagnosticInputRevisions::default();
        let familiar_header = header(1);
        let familiar_record = producer_record(
            TaskDiagnosticProducer::FamiliarDelegation,
            &[TaskDiagnosticClass::PolicyDisabled],
            0,
            false,
        );
        let zero_auto = producer_header(TaskDiagnosticProducer::BlueprintAutoBuild, 0);

        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Build,
                0,
                true,
                TaskStatusEvidence {
                    familiar_header: Some(&familiar_header),
                    familiar_record: Some(&familiar_record),
                    auto_build_header: Some(&zero_auto),
                    auto_build_record: None,
                    revisions: &revisions,
                },
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar)
        );

        let auto_header = producer_header(TaskDiagnosticProducer::BlueprintAutoBuild, 1);
        let auto_record = producer_record(
            TaskDiagnosticProducer::BlueprintAutoBuild,
            &[TaskDiagnosticClass::DependencyWaiting],
            0,
            false,
        );
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Build,
                0,
                true,
                TaskStatusEvidence {
                    familiar_header: Some(&familiar_header),
                    familiar_record: Some(&familiar_record),
                    auto_build_header: Some(&auto_header),
                    auto_build_record: Some(&auto_record),
                    revisions: &revisions,
                },
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::DependencyWaiting)
        );
    }

    #[test]
    fn submitted_partial_and_missing_multi_producer_evidence_stay_pending() {
        let revisions = TaskDiagnosticInputRevisions::default();
        let familiar_header = header(1);
        let submitted = producer_record(TaskDiagnosticProducer::FamiliarDelegation, &[], 1, false);
        let partial = producer_record(
            TaskDiagnosticProducer::FamiliarDelegation,
            &[TaskDiagnosticClass::PolicyDisabled],
            0,
            true,
        );
        for record in [&submitted, &partial] {
            assert_eq!(
                derive_task_status(
                    entity(),
                    WorkType::Chop,
                    0,
                    false,
                    familiar_evidence(Some(&familiar_header), Some(record), &revisions),
                ),
                TaskStatusSummary::PendingEvaluation
            );
        }

        let policy = producer_record(
            TaskDiagnosticProducer::FamiliarDelegation,
            &[TaskDiagnosticClass::PolicyDisabled],
            0,
            false,
        );
        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Build,
                0,
                true,
                TaskStatusEvidence {
                    familiar_header: Some(&familiar_header),
                    familiar_record: Some(&policy),
                    auto_build_header: None,
                    auto_build_record: None,
                    revisions: &revisions,
                },
            ),
            TaskStatusSummary::PendingEvaluation
        );
    }

    #[test]
    fn unrelated_availability_change_keeps_zero_roster_evidence_current() {
        let mut revisions = TaskDiagnosticInputRevisions::default();
        let current_header = header(0);
        revisions.bump_availability();

        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(Some(&current_header), None, &revisions),
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar)
        );
    }

    #[test]
    fn unrelated_availability_change_keeps_roster_only_record_current() {
        let mut revisions = TaskDiagnosticInputRevisions::default();
        let current_header = header(1);
        let mut current_record = record(TaskDiagnosticClass::NoEligibleFamiliar, 0);
        current_record.domains =
            TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::ROSTER);
        revisions.bump_availability();

        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(Some(&current_header), Some(&current_record), &revisions),
            ),
            TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar)
        );
    }

    #[test]
    fn roster_change_invalidates_reason_specific_coverage() {
        let mut revisions = TaskDiagnosticInputRevisions::default();
        let current_header = header(1);
        let mut current_record = record(TaskDiagnosticClass::Unreachable, 0);
        current_record.domains =
            TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY);
        revisions.bump_roster();

        assert_eq!(
            derive_task_status(
                entity(),
                WorkType::Chop,
                0,
                false,
                familiar_evidence(Some(&current_header), Some(&current_record), &revisions),
            ),
            TaskStatusSummary::PendingEvaluation
        );
    }
}
