use std::collections::HashMap;

use bevy::prelude::*;
use hw_jobs::{
    TaskDiagnosticClass, TaskDiagnosticCounters, TaskDiagnosticCoverage, TaskDiagnosticCycleHeader,
    TaskDiagnosticDomainMask, TaskDiagnosticInputRevisions, TaskDiagnosticInputStamp,
    TaskDiagnosticProducer, TaskDiagnosticRecord,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum AutoBuildLocalOutcome {
    Submitted,
    Rejected(TaskDiagnosticClass),
    Partial,
}

#[derive(Debug)]
pub(super) struct AutoBuildEvaluator {
    records: HashMap<Entity, (TaskDiagnosticInputStamp, AutoBuildLocalOutcome)>,
}

impl AutoBuildEvaluator {
    pub(super) fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    pub(super) fn observe(&mut self, task: Entity, revisions: &TaskDiagnosticInputRevisions) {
        self.records
            .entry(task)
            .or_insert((revisions.stamp_for(task), AutoBuildLocalOutcome::Partial));
    }

    pub(super) fn set(&mut self, task: Entity, outcome: AutoBuildLocalOutcome) {
        if let Some((_, current)) = self.records.get_mut(&task) {
            // A real submission is the strongest local evidence.
            if !matches!(current, AutoBuildLocalOutcome::Submitted) {
                *current = outcome;
            }
        }
    }
}

pub(super) struct BlueprintAutoBuildDiagnosticCycle {
    header: TaskDiagnosticCycleHeader,
    records: HashMap<Entity, TaskDiagnosticRecord>,
}

impl BlueprintAutoBuildDiagnosticCycle {
    pub(super) fn new(cycle: u64, revisions: &TaskDiagnosticInputRevisions) -> Self {
        Self {
            header: TaskDiagnosticCycleHeader {
                producer: TaskDiagnosticProducer::BlueprintAutoBuild,
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

    pub(super) fn begin_evaluator(&mut self) {
        self.header.eligible_evaluators = self.header.eligible_evaluators.saturating_add(1);
    }

    pub(super) fn finish_evaluator(&mut self, evaluator: AutoBuildEvaluator) {
        self.header.completed_evaluators = self.header.completed_evaluators.saturating_add(1);
        for (task, (stamp, outcome)) in evaluator.records {
            let record = self
                .records
                .entry(task)
                .or_insert_with(|| TaskDiagnosticRecord {
                    producer: TaskDiagnosticProducer::BlueprintAutoBuild,
                    coverage: TaskDiagnosticCoverage::default(),
                    counters: TaskDiagnosticCounters::default(),
                    stamp,
                    domains: TaskDiagnosticDomainMask::TASK,
                });
            record.coverage.applicable_evaluators =
                record.coverage.applicable_evaluators.saturating_add(1);
            if record.stamp != stamp {
                record.coverage.partial = true;
                continue;
            }

            match outcome {
                AutoBuildLocalOutcome::Submitted => {
                    record.domains = record.domains.union(TaskDiagnosticDomainMask::ROSTER);
                    record.coverage.evaluated_evaluators =
                        record.coverage.evaluated_evaluators.saturating_add(1);
                    record.coverage.submitted_count =
                        record.coverage.submitted_count.saturating_add(1);
                }
                AutoBuildLocalOutcome::Rejected(class) => {
                    record.domains = record
                        .domains
                        .union(TaskDiagnosticDomainMask::for_class(class));
                    record.coverage.evaluated_evaluators =
                        record.coverage.evaluated_evaluators.saturating_add(1);
                    record.coverage.terminal_votes =
                        record.coverage.terminal_votes.saturating_add(1);
                    record.counters.increment(class);
                }
                AutoBuildLocalOutcome::Partial => record.coverage.partial = true,
            }
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct BlueprintAutoBuildDiagnostics {
    header: Option<TaskDiagnosticCycleHeader>,
    records: HashMap<Entity, TaskDiagnosticRecord>,
}

impl BlueprintAutoBuildDiagnostics {
    pub(super) fn next_cycle(&self) -> u64 {
        self.header.map_or(1, |header| header.cycle.wrapping_add(1))
    }

    pub(super) fn publish(&mut self, cycle: BlueprintAutoBuildDiagnosticCycle) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_replaces_the_previous_cycle() {
        let task = Entity::from_raw_u32(3).expect("valid test entity");
        let revisions = TaskDiagnosticInputRevisions::default();
        let mut first = BlueprintAutoBuildDiagnosticCycle::new(1, &revisions);
        first.begin_evaluator();
        let mut evaluator = AutoBuildEvaluator::new();
        evaluator.observe(task, &revisions);
        evaluator.set(
            task,
            AutoBuildLocalOutcome::Rejected(TaskDiagnosticClass::DependencyWaiting),
        );
        first.finish_evaluator(evaluator);

        let mut published = BlueprintAutoBuildDiagnostics::default();
        published.publish(first);
        assert!(published.record(task).is_some());

        published.publish(BlueprintAutoBuildDiagnosticCycle::new(2, &revisions));
        assert!(published.record(task).is_none());
    }
}
