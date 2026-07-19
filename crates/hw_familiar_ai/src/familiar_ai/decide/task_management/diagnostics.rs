//! Latest-only diagnostics emitted as a by-product of normal delegation.

use std::collections::HashMap;

use bevy::prelude::*;
use hw_jobs::{
    TaskDiagnosticClass, TaskDiagnosticCounters, TaskDiagnosticCoverage, TaskDiagnosticCycleHeader,
    TaskDiagnosticDomainMask, TaskDiagnosticInputRevisions, TaskDiagnosticInputStamp,
    TaskDiagnosticProducer, TaskDiagnosticRecord,
};

/// Internal branch classification. Variants without a player-safe mapping keep
/// coverage partial instead of inventing a blocker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateRejectReason {
    NoEligibleFamiliar,
    MissingResourceOrSource,
    Unreachable,
    TemporaryContention,
    DependencyWaiting,
    MalformedTask,
    StaleInput,
    Unevaluated,
}

impl CandidateRejectReason {
    #[must_use]
    pub const fn diagnostic_class(self) -> Option<TaskDiagnosticClass> {
        match self {
            Self::NoEligibleFamiliar => Some(TaskDiagnosticClass::NoEligibleFamiliar),
            Self::MissingResourceOrSource => Some(TaskDiagnosticClass::MissingResourceOrSource),
            Self::Unreachable => Some(TaskDiagnosticClass::Unreachable),
            Self::TemporaryContention => Some(TaskDiagnosticClass::TemporaryContention),
            Self::DependencyWaiting => Some(TaskDiagnosticClass::DependencyWaiting),
            Self::MalformedTask | Self::StaleInput | Self::Unevaluated => None,
        }
    }
}

/// Result of the final assignment policy, kept typed until the evaluator
/// reducer consumes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAssignmentAttempt {
    Submitted,
    Rejected(CandidateRejectReason),
}

#[derive(Debug, Clone)]
struct LocalTaskDiagnostic {
    stamp: TaskDiagnosticInputStamp,
    reasons: TaskDiagnosticCounters,
    submitted: bool,
    partial: bool,
}

impl LocalTaskDiagnostic {
    fn new(stamp: TaskDiagnosticInputStamp) -> Self {
        Self {
            stamp,
            reasons: TaskDiagnosticCounters::default(),
            submitted: false,
            partial: false,
        }
    }
}

/// Per-Familiar reducer. Worker/source branch multiplicity is collapsed to
/// reason presence before it is merged into the published task record.
pub struct FamiliarEvaluatorDiagnostics {
    idle_worker_count: usize,
    records: HashMap<Entity, LocalTaskDiagnostic>,
}

impl FamiliarEvaluatorDiagnostics {
    #[must_use]
    pub fn new(idle_worker_count: usize) -> Self {
        Self {
            idle_worker_count,
            records: HashMap::new(),
        }
    }

    pub fn observe_applicable(&mut self, task: Entity, revisions: &TaskDiagnosticInputRevisions) {
        self.records
            .entry(task)
            .or_insert_with(|| LocalTaskDiagnostic::new(revisions.stamp_for(task)));
    }

    pub fn set_idle_worker_count(&mut self, idle_worker_count: usize) {
        self.idle_worker_count = idle_worker_count;
    }

    pub fn reject(&mut self, task: Entity, reason: CandidateRejectReason) {
        let Some(record) = self.records.get_mut(&task) else {
            return;
        };
        if let Some(class) = reason.diagnostic_class() {
            // Presence, rather than branch hit count, is the local vote input.
            if record.reasons.count(class) == 0 {
                record.reasons.increment(class);
            }
        } else {
            record.partial = true;
        }
    }

    pub fn mark_partial(&mut self, task: Entity) {
        if let Some(record) = self.records.get_mut(&task) {
            record.partial = true;
        }
    }

    pub fn mark_submitted(&mut self, task: Entity) {
        if let Some(record) = self.records.get_mut(&task) {
            record.submitted = true;
        }
    }
}

/// Mutable accumulator for one normal delegation cycle.
pub struct FamiliarTaskDiagnosticCycle {
    header: TaskDiagnosticCycleHeader,
    records: HashMap<Entity, TaskDiagnosticRecord>,
}

impl FamiliarTaskDiagnosticCycle {
    #[must_use]
    pub fn new(cycle: u64, revisions: &TaskDiagnosticInputRevisions) -> Self {
        Self {
            header: TaskDiagnosticCycleHeader {
                producer: TaskDiagnosticProducer::FamiliarDelegation,
                cycle,
                eligible_evaluators: 0,
                completed_evaluators: 0,
                stamp: TaskDiagnosticInputStamp {
                    task: 0,
                    roster: revisions.roster,
                    availability: revisions.availability,
                    topology: revisions.topology,
                },
            },
            records: HashMap::new(),
        }
    }

    pub fn begin_evaluator(&mut self) {
        self.header.eligible_evaluators = self.header.eligible_evaluators.saturating_add(1);
    }

    pub fn finish_evaluator(&mut self, evaluator: FamiliarEvaluatorDiagnostics) {
        self.header.completed_evaluators = self.header.completed_evaluators.saturating_add(1);

        for (task, local) in evaluator.records {
            let record = self
                .records
                .entry(task)
                .or_insert_with(|| TaskDiagnosticRecord {
                    producer: TaskDiagnosticProducer::FamiliarDelegation,
                    coverage: TaskDiagnosticCoverage::default(),
                    counters: TaskDiagnosticCounters::default(),
                    stamp: local.stamp,
                    domains: TaskDiagnosticDomainMask::TASK,
                });
            record.coverage.applicable_evaluators =
                record.coverage.applicable_evaluators.saturating_add(1);

            if record.stamp != local.stamp {
                record.coverage.partial = true;
                continue;
            }

            // The local contract is deliberately ordered: submitted, partial,
            // zero worker, then fixed-precedence reason presence.
            if local.submitted {
                record.domains = record
                    .domains
                    .union(TaskDiagnosticDomainMask::TASK)
                    .union(TaskDiagnosticDomainMask::ROSTER);
                record.coverage.evaluated_evaluators =
                    record.coverage.evaluated_evaluators.saturating_add(1);
                record.coverage.submitted_count = record.coverage.submitted_count.saturating_add(1);
                continue;
            }
            if local.partial {
                record.coverage.partial = true;
                continue;
            }

            let representative = if evaluator.idle_worker_count == 0 {
                Some(TaskDiagnosticClass::NoEligibleFamiliar)
            } else {
                local.reasons.representative()
            };
            let Some(representative) = representative else {
                record.coverage.partial = true;
                continue;
            };

            record.coverage.evaluated_evaluators =
                record.coverage.evaluated_evaluators.saturating_add(1);
            record.coverage.terminal_votes = record.coverage.terminal_votes.saturating_add(1);
            record.counters.increment(representative);
            record.domains = record
                .domains
                .union(TaskDiagnosticDomainMask::for_class(representative));
        }
    }
}

/// Latest-only familiar producer snapshot. Publishing replaces the previous
/// map, so removed tasks and stale cycles cannot accumulate.
#[derive(Resource, Debug, Default)]
pub struct FamiliarTaskCandidateDiagnostics {
    header: Option<TaskDiagnosticCycleHeader>,
    records: HashMap<Entity, TaskDiagnosticRecord>,
}

impl FamiliarTaskCandidateDiagnostics {
    #[must_use]
    pub fn next_cycle(&self) -> u64 {
        self.header.map_or(1, |header| header.cycle.wrapping_add(1))
    }

    pub fn publish(&mut self, cycle: FamiliarTaskDiagnosticCycle) {
        self.header = Some(cycle.header);
        self.records = cycle.records;
    }

    #[must_use]
    pub const fn header(&self) -> Option<&TaskDiagnosticCycleHeader> {
        self.header.as_ref()
    }

    #[must_use]
    pub fn record(&self, task: Entity) -> Option<&TaskDiagnosticRecord> {
        self.records.get(&task)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid test entity")
    }

    #[test]
    fn local_reason_multiplicity_does_not_change_its_single_vote() {
        let task = entity(1);
        let revisions = TaskDiagnosticInputRevisions::default();
        let mut evaluator = FamiliarEvaluatorDiagnostics::new(3);
        evaluator.observe_applicable(task, &revisions);
        evaluator.reject(task, CandidateRejectReason::TemporaryContention);
        evaluator.reject(task, CandidateRejectReason::TemporaryContention);
        evaluator.reject(task, CandidateRejectReason::MissingResourceOrSource);

        let mut cycle = FamiliarTaskDiagnosticCycle::new(1, &revisions);
        cycle.begin_evaluator();
        cycle.finish_evaluator(evaluator);
        let mut published = FamiliarTaskCandidateDiagnostics::default();
        published.publish(cycle);

        let record = published.record(task).expect("task was observed");
        assert_eq!(record.coverage.terminal_votes, 1);
        assert_eq!(record.counters.total(), 1);
        assert_eq!(
            record.counters.representative(),
            Some(TaskDiagnosticClass::MissingResourceOrSource)
        );
        assert!(
            record
                .domains
                .contains(TaskDiagnosticDomainMask::AVAILABILITY)
        );
        assert!(!record.domains.contains(TaskDiagnosticDomainMask::ROSTER));
    }

    #[test]
    fn submitted_precedes_partial_but_does_not_become_a_terminal_vote() {
        let task = entity(1);
        let revisions = TaskDiagnosticInputRevisions::default();
        let mut evaluator = FamiliarEvaluatorDiagnostics::new(1);
        evaluator.observe_applicable(task, &revisions);
        evaluator.mark_partial(task);
        evaluator.mark_submitted(task);

        let mut cycle = FamiliarTaskDiagnosticCycle::new(1, &revisions);
        cycle.begin_evaluator();
        cycle.finish_evaluator(evaluator);
        let record = cycle.records.get(&task).expect("task was observed");

        assert_eq!(record.coverage.submitted_count, 1);
        assert_eq!(record.coverage.terminal_votes, 0);
        assert!(!record.coverage.partial);
    }

    #[test]
    fn zero_worker_is_normalized_to_no_eligible_familiar() {
        let task = entity(1);
        let revisions = TaskDiagnosticInputRevisions::default();
        let mut evaluator = FamiliarEvaluatorDiagnostics::new(0);
        evaluator.observe_applicable(task, &revisions);
        evaluator.reject(task, CandidateRejectReason::MissingResourceOrSource);

        let mut cycle = FamiliarTaskDiagnosticCycle::new(1, &revisions);
        cycle.begin_evaluator();
        cycle.finish_evaluator(evaluator);
        let record = cycle.records.get(&task).expect("task was observed");

        assert_eq!(
            record.counters.representative(),
            Some(TaskDiagnosticClass::NoEligibleFamiliar)
        );
        assert!(record.domains.contains(TaskDiagnosticDomainMask::ROSTER));
        assert!(
            !record
                .domains
                .contains(TaskDiagnosticDomainMask::AVAILABILITY)
        );
    }

    #[test]
    fn publishing_a_cycle_replaces_old_task_entities() {
        let revisions = TaskDiagnosticInputRevisions::default();
        let mut published = FamiliarTaskCandidateDiagnostics::default();

        let mut first = FamiliarTaskDiagnosticCycle::new(1, &revisions);
        first.begin_evaluator();
        let mut evaluator = FamiliarEvaluatorDiagnostics::new(0);
        evaluator.observe_applicable(entity(1), &revisions);
        first.finish_evaluator(evaluator);
        published.publish(first);

        published.publish(FamiliarTaskDiagnosticCycle::new(2, &revisions));
        assert!(published.is_empty());
        assert_eq!(published.header().map(|header| header.cycle), Some(2));
    }

    #[test]
    fn published_map_scales_with_current_tasks_not_evaluator_history() {
        let revisions = TaskDiagnosticInputRevisions::default();
        let mut published = FamiliarTaskCandidateDiagnostics::default();

        let mut first = FamiliarTaskDiagnosticCycle::new(1, &revisions);
        for evaluator_index in 0..4 {
            first.begin_evaluator();
            let mut evaluator = FamiliarEvaluatorDiagnostics::new(1);
            for task_index in 1..=128 {
                let task = entity(task_index);
                evaluator.observe_applicable(task, &revisions);
                evaluator.reject(task, CandidateRejectReason::NoEligibleFamiliar);
            }
            assert_eq!(evaluator.records.len(), 128, "evaluator {evaluator_index}");
            first.finish_evaluator(evaluator);
        }
        published.publish(first);
        assert_eq!(published.len(), 128);

        let mut second = FamiliarTaskDiagnosticCycle::new(2, &revisions);
        second.begin_evaluator();
        let mut evaluator = FamiliarEvaluatorDiagnostics::new(1);
        for task_index in 1..=16 {
            let task = entity(task_index);
            evaluator.observe_applicable(task, &revisions);
            evaluator.reject(task, CandidateRejectReason::NoEligibleFamiliar);
        }
        second.finish_evaluator(evaluator);
        published.publish(second);

        assert_eq!(published.len(), 16);
        assert!(published.record(entity(17)).is_none());
    }
}
