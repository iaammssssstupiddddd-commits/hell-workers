//! Shared, presentation-neutral task diagnostic contracts.
//!
//! Producer crates publish latest-only observations with these fixed-width
//! types.  The root UI adapter is responsible for combining every applicable
//! producer; a missing or stale producer must therefore remain pending.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::WorkType;

/// Stable, player-safe diagnostic classes shared by task producers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TaskDiagnosticClass {
    NoEligibleFamiliar = 0,
    MissingResourceOrSource = 1,
    Unreachable = 2,
    TemporaryContention = 3,
    DependencyWaiting = 4,
}

impl TaskDiagnosticClass {
    pub const COUNT: usize = 5;

    /// Tie-break order used after normalized producer votes are counted.
    pub const REPRESENTATIVE_ORDER: [Self; Self::COUNT] = [
        Self::MissingResourceOrSource,
        Self::NoEligibleFamiliar,
        Self::Unreachable,
        Self::DependencyWaiting,
        Self::TemporaryContention,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Fixed-width vote storage. One producer/evaluator contributes at most one
/// terminal vote, so worker and source counts cannot bias the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskDiagnosticCounters([u16; TaskDiagnosticClass::COUNT]);

impl TaskDiagnosticCounters {
    pub fn increment(&mut self, class: TaskDiagnosticClass) {
        let count = &mut self.0[class.index()];
        *count = count.saturating_add(1);
    }

    pub fn merge(&mut self, other: &Self) {
        for (target, source) in self.0.iter_mut().zip(other.0) {
            *target = target.saturating_add(source);
        }
    }

    #[must_use]
    pub const fn count(&self, class: TaskDiagnosticClass) -> u16 {
        self.0[class.index()]
    }

    #[must_use]
    pub fn total(&self) -> u16 {
        self.0.iter().copied().fold(0u16, u16::saturating_add)
    }

    #[must_use]
    pub fn representative(&self) -> Option<TaskDiagnosticClass> {
        let maximum = self.0.iter().copied().max().unwrap_or(0);
        if maximum == 0 {
            return None;
        }

        TaskDiagnosticClass::REPRESENTATIVE_ORDER
            .into_iter()
            .find(|class| self.count(*class) == maximum)
    }
}

/// Assignment paths that can independently accept the same designation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TaskDiagnosticProducer {
    FamiliarDelegation = 0,
    BlueprintAutoBuild = 1,
}

impl TaskDiagnosticProducer {
    pub const COUNT: usize = 2;

    #[must_use]
    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Applicable producer mask for a task kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDiagnosticProducerMask(u8);

impl TaskDiagnosticProducerMask {
    pub const FAMILIAR: Self = Self(TaskDiagnosticProducer::FamiliarDelegation.bit());
    pub const BUILD: Self = Self(
        TaskDiagnosticProducer::FamiliarDelegation.bit()
            | TaskDiagnosticProducer::BlueprintAutoBuild.bit(),
    );

    #[must_use]
    pub const fn for_work_type(work_type: WorkType) -> Self {
        if matches!(work_type, WorkType::Build) {
            Self::BUILD
        } else {
            Self::FAMILIAR
        }
    }

    /// Chooses producers from the live task shape. Blueprint auto-build is
    /// only applicable to an unowned Blueprint; a Familiar-owned build must
    /// not wait forever for an evaluator that intentionally skips it.
    #[must_use]
    pub const fn for_task(work_type: WorkType, auto_build_applicable: bool) -> Self {
        if matches!(work_type, WorkType::Build) && auto_build_applicable {
            Self::BUILD
        } else {
            Self::FAMILIAR
        }
    }

    #[must_use]
    pub const fn contains(self, producer: TaskDiagnosticProducer) -> bool {
        self.0 & producer.bit() != 0
    }
}

/// Input domains used by a diagnostic result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskDiagnosticDomainMask(u8);

impl TaskDiagnosticDomainMask {
    pub const TASK: Self = Self(1 << 0);
    pub const ROSTER: Self = Self(1 << 1);
    pub const AVAILABILITY: Self = Self(1 << 2);
    pub const TOPOLOGY: Self = Self(1 << 3);
    pub const ALL: Self =
        Self(Self::TASK.0 | Self::ROSTER.0 | Self::AVAILABILITY.0 | Self::TOPOLOGY.0);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Semantic inputs required to keep a published blocker current.
    #[must_use]
    pub const fn for_class(class: TaskDiagnosticClass) -> Self {
        match class {
            TaskDiagnosticClass::NoEligibleFamiliar => Self::TASK.union(Self::ROSTER),
            TaskDiagnosticClass::MissingResourceOrSource
            | TaskDiagnosticClass::TemporaryContention => Self::TASK.union(Self::AVAILABILITY),
            TaskDiagnosticClass::DependencyWaiting => Self::TASK,
            TaskDiagnosticClass::Unreachable => Self::TASK.union(Self::TOPOLOGY),
        }
    }
}

/// Semantic revisions captured by a producer when it evaluates a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskDiagnosticInputStamp {
    pub task: u64,
    pub roster: u64,
    pub availability: u64,
    pub topology: u64,
}

/// Current semantic revisions. Runtime-only; it is intentionally not part of
/// the save schema.
#[derive(Resource, Debug, Default)]
pub struct TaskDiagnosticInputRevisions {
    task: HashMap<Entity, u64>,
    pub roster: u64,
    pub availability: u64,
    pub topology: u64,
}

impl TaskDiagnosticInputRevisions {
    #[must_use]
    pub fn stamp_for(&self, task: Entity) -> TaskDiagnosticInputStamp {
        TaskDiagnosticInputStamp {
            task: self.task_revision(task),
            roster: self.roster,
            availability: self.availability,
            topology: self.topology,
        }
    }

    #[must_use]
    pub fn task_revision(&self, task: Entity) -> u64 {
        self.task.get(&task).copied().unwrap_or(0)
    }

    pub fn bump_task(&mut self, task: Entity) {
        let revision = self.task.entry(task).or_default();
        *revision = revision.wrapping_add(1);
    }

    pub fn remove_task(&mut self, task: Entity) {
        self.task.remove(&task);
    }

    pub fn bump_roster(&mut self) {
        self.roster = self.roster.wrapping_add(1);
    }

    pub fn bump_availability(&mut self) {
        self.availability = self.availability.wrapping_add(1);
    }

    pub fn set_topology(&mut self, topology: u64) {
        self.topology = topology;
    }

    #[must_use]
    pub fn is_current(
        &self,
        task: Entity,
        stamp: TaskDiagnosticInputStamp,
        domains: TaskDiagnosticDomainMask,
    ) -> bool {
        (!domains.contains(TaskDiagnosticDomainMask::TASK)
            || stamp.task == self.task_revision(task))
            && (!domains.contains(TaskDiagnosticDomainMask::ROSTER) || stamp.roster == self.roster)
            && (!domains.contains(TaskDiagnosticDomainMask::AVAILABILITY)
                || stamp.availability == self.availability)
            && (!domains.contains(TaskDiagnosticDomainMask::TOPOLOGY)
                || stamp.topology == self.topology)
    }
}

/// Fixed-width coverage summary for one task and one producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskDiagnosticCoverage {
    pub applicable_evaluators: u16,
    pub evaluated_evaluators: u16,
    pub terminal_votes: u16,
    pub submitted_count: u16,
    pub partial: bool,
}

impl TaskDiagnosticCoverage {
    #[must_use]
    pub fn is_complete_rejection(&self) -> bool {
        self.applicable_evaluators > 0
            && !self.partial
            && self.submitted_count == 0
            && self.evaluated_evaluators == self.applicable_evaluators
            && self.terminal_votes == self.applicable_evaluators
    }
}

/// Latest observation for one task from one producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDiagnosticRecord {
    pub producer: TaskDiagnosticProducer,
    pub coverage: TaskDiagnosticCoverage,
    pub counters: TaskDiagnosticCounters,
    pub stamp: TaskDiagnosticInputStamp,
    pub domains: TaskDiagnosticDomainMask,
}

/// Producer-cycle metadata. A zero-roster cycle still publishes this header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDiagnosticCycleHeader {
    pub producer: TaskDiagnosticProducer,
    pub cycle: u64,
    pub eligible_evaluators: u16,
    pub completed_evaluators: u16,
    pub stamp: TaskDiagnosticInputStamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_saturate_and_use_fixed_representative_tie_break() {
        let mut counters = TaskDiagnosticCounters::default();
        for _ in 0..=u16::MAX {
            counters.increment(TaskDiagnosticClass::NoEligibleFamiliar);
        }
        assert_eq!(
            counters.count(TaskDiagnosticClass::NoEligibleFamiliar),
            u16::MAX
        );

        let mut tie = TaskDiagnosticCounters::default();
        tie.increment(TaskDiagnosticClass::TemporaryContention);
        tie.increment(TaskDiagnosticClass::MissingResourceOrSource);
        assert_eq!(
            tie.representative(),
            Some(TaskDiagnosticClass::MissingResourceOrSource)
        );
    }

    #[test]
    fn partial_or_submitted_coverage_is_not_a_complete_rejection() {
        let complete = TaskDiagnosticCoverage {
            applicable_evaluators: 1,
            evaluated_evaluators: 1,
            terminal_votes: 1,
            submitted_count: 0,
            partial: false,
        };
        assert!(complete.is_complete_rejection());
        assert!(
            !TaskDiagnosticCoverage {
                partial: true,
                ..complete
            }
            .is_complete_rejection()
        );
        assert!(
            !TaskDiagnosticCoverage {
                submitted_count: 1,
                ..complete
            }
            .is_complete_rejection()
        );
    }

    #[test]
    fn build_requires_both_assignment_producers() {
        let build = TaskDiagnosticProducerMask::for_work_type(WorkType::Build);
        assert!(build.contains(TaskDiagnosticProducer::FamiliarDelegation));
        assert!(build.contains(TaskDiagnosticProducer::BlueprintAutoBuild));

        let chop = TaskDiagnosticProducerMask::for_work_type(WorkType::Chop);
        assert!(chop.contains(TaskDiagnosticProducer::FamiliarDelegation));
        assert!(!chop.contains(TaskDiagnosticProducer::BlueprintAutoBuild));

        let managed_build = TaskDiagnosticProducerMask::for_task(WorkType::Build, false);
        assert!(managed_build.contains(TaskDiagnosticProducer::FamiliarDelegation));
        assert!(!managed_build.contains(TaskDiagnosticProducer::BlueprintAutoBuild));
    }

    #[test]
    fn only_used_revision_domains_invalidate_a_record() {
        let task = Entity::from_raw_u32(7).expect("valid test entity");
        let mut revisions = TaskDiagnosticInputRevisions::default();
        let stamp = revisions.stamp_for(task);

        revisions.bump_availability();
        assert!(revisions.is_current(task, stamp, TaskDiagnosticDomainMask::ROSTER));
        assert!(!revisions.is_current(task, stamp, TaskDiagnosticDomainMask::AVAILABILITY));
    }

    #[test]
    fn dependency_waiting_only_uses_task_local_revision() {
        let domains = TaskDiagnosticDomainMask::for_class(TaskDiagnosticClass::DependencyWaiting);

        assert!(domains.contains(TaskDiagnosticDomainMask::TASK));
        assert!(!domains.contains(TaskDiagnosticDomainMask::AVAILABILITY));
        assert!(!domains.contains(TaskDiagnosticDomainMask::ROSTER));
        assert!(!domains.contains(TaskDiagnosticDomainMask::TOPOLOGY));
    }

    #[test]
    fn published_task_records_are_fixed_width_and_heap_free() {
        assert_eq!(
            std::mem::size_of::<TaskDiagnosticCounters>(),
            std::mem::size_of::<u16>() * TaskDiagnosticClass::COUNT
        );
        assert!(
            !std::mem::needs_drop::<TaskDiagnosticRecord>(),
            "a task record must not retain a task-by-evaluator heap collection",
        );
    }
}
