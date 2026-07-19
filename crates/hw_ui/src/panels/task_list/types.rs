use std::cmp::Ordering;

use bevy::prelude::*;
use hw_core::jobs::WorkType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskBlockerReason {
    NoEligibleFamiliar,
    MissingResourceOrSource,
    Unreachable,
    TemporaryContention,
    DependencyWaiting,
}

impl TaskBlockerReason {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoEligibleFamiliar => "No eligible familiar",
            Self::MissingResourceOrSource => "Missing resource or source",
            Self::Unreachable => "Unreachable",
            Self::TemporaryContention => "Waiting for reservation",
            Self::DependencyWaiting => "Waiting for dependency",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatusSummary {
    Working,
    Blocked(TaskBlockerReason),
    PendingEvaluation,
}

impl TaskStatusSummary {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Working => "Working".to_string(),
            Self::Blocked(reason) => format!("Blocked: {}", reason.label()),
            Self::PendingEvaluation => "Evaluating...".to_string(),
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Working => 0,
            Self::Blocked(_) => 1,
            Self::PendingEvaluation => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriorityTier {
    Normal,
    High,
    Critical,
}

impl TaskPriorityTier {
    #[must_use]
    pub const fn from_priority(priority: u32) -> Self {
        match priority {
            0..=4 => Self::Normal,
            5..=9 => Self::High,
            _ => Self::Critical,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }

    #[must_use]
    pub const fn adjusted_value(self, adjustment: TaskPriorityAdjustment) -> u32 {
        match (self, adjustment) {
            (Self::Normal, TaskPriorityAdjustment::Decrease) => 0,
            (Self::Normal, TaskPriorityAdjustment::Increase) => 5,
            (Self::High, TaskPriorityAdjustment::Decrease) => 0,
            (Self::High, TaskPriorityAdjustment::Increase) => 10,
            (Self::Critical, TaskPriorityAdjustment::Decrease) => 5,
            (Self::Critical, TaskPriorityAdjustment::Increase) => 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskPriorityAdjustment {
    Decrease,
    Increase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskCancelKind {
    GenericDesignation,
    Blueprint,
    ManualTransportRequest,
    FloorSite(Entity),
    WallSite(Entity),
}

impl TaskCancelKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FloorSite(_) | Self::WallSite(_) => "Cancel site",
            Self::GenericDesignation | Self::Blueprint | Self::ManualTransportRequest => "Cancel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskActionCapabilities {
    pub focus: bool,
    pub priority: bool,
    pub cancel: Option<TaskCancelKind>,
}

impl TaskActionCapabilities {
    pub const READ_ONLY: Self = Self {
        focus: true,
        priority: false,
        cancel: None,
    };

    #[must_use]
    pub const fn has_actions(self) -> bool {
        self.priority || self.cancel.is_some()
    }
}

impl Default for TaskActionCapabilities {
    fn default() -> Self {
        Self::READ_ONLY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskWorkTypeFilter {
    #[default]
    All,
    Only(WorkType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskStatusFilter {
    #[default]
    All,
    Working,
    Blocked,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskPriorityFilter {
    #[default]
    All,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskWorkerFilter {
    #[default]
    All,
    Assigned,
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskSortKey {
    #[default]
    WorkType,
    Status,
    Priority,
    WorkerCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskSortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskDashboardViewState {
    pub work_type: TaskWorkTypeFilter,
    pub status: TaskStatusFilter,
    pub priority: TaskPriorityFilter,
    pub workers: TaskWorkerFilter,
    pub sort_key: TaskSortKey,
    pub direction: TaskSortDirection,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDashboardControl {
    WorkTypeFilter,
    StatusFilter,
    PriorityFilter,
    WorkerFilter,
    SortKey,
    SortDirection,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskActionButton {
    pub target: Entity,
    pub expected_work_type: WorkType,
    pub kind: TaskActionButtonKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskActionButtonKind {
    AdjustPriority(TaskPriorityAdjustment),
    Cancel(TaskCancelKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingTaskCancellation {
    pub target: Entity,
    pub expected_work_type: WorkType,
    pub kind: TaskCancelKind,
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskDashboardActionState {
    pub confirmation: Option<PendingTaskCancellation>,
}

/// Marks every node generated beneath `TaskListBody`, including headers and
/// empty-state rows that do not carry `TaskListItem`.
#[derive(Component)]
pub struct TaskListDynamicNode;

/// Task dashboard entry after root-owned game state has been adapted to UI
/// safe values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEntry {
    pub entity: Entity,
    pub work_type: WorkType,
    pub description: String,
    pub priority: u32,
    pub worker_count: usize,
    pub status: TaskStatusSummary,
    pub actions: TaskActionCapabilities,
}

impl TaskEntry {
    #[must_use]
    pub const fn priority_tier(&self) -> TaskPriorityTier {
        TaskPriorityTier::from_priority(self.priority)
    }
}

impl TaskDashboardViewState {
    #[must_use]
    pub fn visible_entries<'a>(&self, entries: &'a [TaskEntry]) -> Vec<&'a TaskEntry> {
        let mut visible: Vec<_> = entries.iter().filter(|entry| self.matches(entry)).collect();
        visible.sort_unstable_by(|left, right| {
            let ordering = compare_entries(left, right, self.sort_key);
            match self.direction {
                TaskSortDirection::Ascending => ordering,
                TaskSortDirection::Descending => ordering.reverse(),
            }
            .then_with(|| compare_entity_keys(left.entity, right.entity))
        });
        visible
    }

    fn matches(&self, entry: &TaskEntry) -> bool {
        let work_type_matches = match self.work_type {
            TaskWorkTypeFilter::All => true,
            TaskWorkTypeFilter::Only(work_type) => entry.work_type == work_type,
        };
        let status_matches = match self.status {
            TaskStatusFilter::All => true,
            TaskStatusFilter::Working => entry.status == TaskStatusSummary::Working,
            TaskStatusFilter::Blocked => matches!(entry.status, TaskStatusSummary::Blocked(_)),
            TaskStatusFilter::Pending => entry.status == TaskStatusSummary::PendingEvaluation,
        };
        let priority_matches = match self.priority {
            TaskPriorityFilter::All => true,
            TaskPriorityFilter::Normal => entry.priority_tier() == TaskPriorityTier::Normal,
            TaskPriorityFilter::High => entry.priority_tier() == TaskPriorityTier::High,
            TaskPriorityFilter::Critical => entry.priority_tier() == TaskPriorityTier::Critical,
        };
        let worker_matches = match self.workers {
            TaskWorkerFilter::All => true,
            TaskWorkerFilter::Assigned => entry.worker_count > 0,
            TaskWorkerFilter::Unassigned => entry.worker_count == 0,
        };
        work_type_matches && status_matches && priority_matches && worker_matches
    }

    pub fn apply_control(&mut self, control: TaskDashboardControl) {
        match control {
            TaskDashboardControl::WorkTypeFilter => {
                self.work_type = next_work_type_filter(self.work_type);
            }
            TaskDashboardControl::StatusFilter => {
                self.status = match self.status {
                    TaskStatusFilter::All => TaskStatusFilter::Working,
                    TaskStatusFilter::Working => TaskStatusFilter::Blocked,
                    TaskStatusFilter::Blocked => TaskStatusFilter::Pending,
                    TaskStatusFilter::Pending => TaskStatusFilter::All,
                };
            }
            TaskDashboardControl::PriorityFilter => {
                self.priority = match self.priority {
                    TaskPriorityFilter::All => TaskPriorityFilter::Normal,
                    TaskPriorityFilter::Normal => TaskPriorityFilter::High,
                    TaskPriorityFilter::High => TaskPriorityFilter::Critical,
                    TaskPriorityFilter::Critical => TaskPriorityFilter::All,
                };
            }
            TaskDashboardControl::WorkerFilter => {
                self.workers = match self.workers {
                    TaskWorkerFilter::All => TaskWorkerFilter::Assigned,
                    TaskWorkerFilter::Assigned => TaskWorkerFilter::Unassigned,
                    TaskWorkerFilter::Unassigned => TaskWorkerFilter::All,
                };
            }
            TaskDashboardControl::SortKey => {
                self.sort_key = match self.sort_key {
                    TaskSortKey::WorkType => TaskSortKey::Status,
                    TaskSortKey::Status => TaskSortKey::Priority,
                    TaskSortKey::Priority => TaskSortKey::WorkerCount,
                    TaskSortKey::WorkerCount => TaskSortKey::WorkType,
                };
            }
            TaskDashboardControl::SortDirection => {
                self.direction = match self.direction {
                    TaskSortDirection::Ascending => TaskSortDirection::Descending,
                    TaskSortDirection::Descending => TaskSortDirection::Ascending,
                };
            }
        }
    }
}

fn compare_entries(left: &TaskEntry, right: &TaskEntry, key: TaskSortKey) -> Ordering {
    match key {
        TaskSortKey::WorkType => (left.work_type as u8).cmp(&(right.work_type as u8)),
        TaskSortKey::Status => left.status.rank().cmp(&right.status.rank()),
        TaskSortKey::Priority => left
            .priority_tier()
            .cmp(&right.priority_tier())
            .then_with(|| left.priority.cmp(&right.priority)),
        TaskSortKey::WorkerCount => left.worker_count.cmp(&right.worker_count),
    }
}

fn compare_entity_keys(left: Entity, right: Entity) -> Ordering {
    left.index_u32().cmp(&right.index_u32()).then_with(|| {
        left.generation()
            .to_bits()
            .cmp(&right.generation().to_bits())
    })
}

const WORK_TYPES: [WorkType; 16] = [
    WorkType::Chop,
    WorkType::Mine,
    WorkType::Build,
    WorkType::Move,
    WorkType::Haul,
    WorkType::HaulToMixer,
    WorkType::GatherWater,
    WorkType::CollectBone,
    WorkType::Refine,
    WorkType::HaulWaterToMixer,
    WorkType::WheelbarrowHaul,
    WorkType::ReinforceFloorTile,
    WorkType::PourFloorTile,
    WorkType::FrameWallTile,
    WorkType::CoatWall,
    WorkType::GeneratePower,
];

fn next_work_type_filter(current: TaskWorkTypeFilter) -> TaskWorkTypeFilter {
    match current {
        TaskWorkTypeFilter::All => TaskWorkTypeFilter::Only(WORK_TYPES[0]),
        TaskWorkTypeFilter::Only(current) => WORK_TYPES
            .iter()
            .position(|work_type| *work_type == current)
            .and_then(|index| WORK_TYPES.get(index + 1).copied())
            .map_or(TaskWorkTypeFilter::All, TaskWorkTypeFilter::Only),
    }
}

/// Task list dirty flags shared by the root view-model adapter and UI widget.
#[derive(Resource, Default)]
pub struct TaskListDirty {
    state_dirty: bool,
    list_dirty: bool,
    summary_dirty: bool,
}

impl TaskListDirty {
    pub fn mark_all(&mut self) {
        self.state_dirty = true;
        self.list_dirty = true;
        self.summary_dirty = true;
    }

    pub fn mark_state(&mut self) {
        self.state_dirty = true;
    }

    pub fn mark_summary(&mut self) {
        self.summary_dirty = true;
    }

    pub fn mark_list(&mut self) {
        self.list_dirty = true;
    }

    pub fn clear_list(&mut self) {
        self.list_dirty = false;
    }

    pub fn clear_state(&mut self) {
        self.state_dirty = false;
    }

    pub fn clear_summary(&mut self) {
        self.summary_dirty = false;
    }

    #[must_use]
    pub fn state_dirty(&self) -> bool {
        self.state_dirty
    }

    #[must_use]
    pub fn list_dirty(&self) -> bool {
        self.list_dirty
    }

    #[must_use]
    pub fn summary_dirty(&self) -> bool {
        self.summary_dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(index: u32, status: TaskStatusSummary, priority: u32, workers: usize) -> TaskEntry {
        entry_with_work_type(index, WorkType::Chop, status, priority, workers)
    }

    fn entry_with_work_type(
        index: u32,
        work_type: WorkType,
        status: TaskStatusSummary,
        priority: u32,
        workers: usize,
    ) -> TaskEntry {
        TaskEntry {
            entity: Entity::from_raw_u32(index).expect("valid test entity"),
            work_type,
            description: format!("task {index}"),
            priority,
            worker_count: workers,
            status,
            actions: TaskActionCapabilities::READ_ONLY,
        }
    }

    #[test]
    fn priority_tier_has_one_shared_threshold_contract() {
        assert_eq!(TaskPriorityTier::from_priority(0), TaskPriorityTier::Normal);
        assert_eq!(TaskPriorityTier::from_priority(4), TaskPriorityTier::Normal);
        assert_eq!(TaskPriorityTier::from_priority(5), TaskPriorityTier::High);
        assert_eq!(TaskPriorityTier::from_priority(9), TaskPriorityTier::High);
        assert_eq!(
            TaskPriorityTier::from_priority(10),
            TaskPriorityTier::Critical
        );
        assert_eq!(
            TaskPriorityTier::from_priority(4).adjusted_value(TaskPriorityAdjustment::Increase),
            5
        );
        assert_eq!(
            TaskPriorityTier::from_priority(9).adjusted_value(TaskPriorityAdjustment::Increase),
            10
        );
        assert_eq!(
            TaskPriorityTier::from_priority(30).adjusted_value(TaskPriorityAdjustment::Decrease),
            5
        );
    }

    #[test]
    fn filters_are_applied_before_stable_entity_ordering() {
        let entries = vec![
            entry(9, TaskStatusSummary::Working, 5, 1),
            entry(2, TaskStatusSummary::Working, 5, 2),
            entry(4, TaskStatusSummary::PendingEvaluation, 0, 0),
        ];
        let state = TaskDashboardViewState {
            status: TaskStatusFilter::Working,
            priority: TaskPriorityFilter::High,
            sort_key: TaskSortKey::Priority,
            ..default()
        };

        assert_eq!(
            state
                .visible_entries(&entries)
                .into_iter()
                .map(|entry| entry.entity.index_u32())
                .collect::<Vec<_>>(),
            vec![2, 9]
        );
    }

    #[test]
    fn descending_sort_keeps_entity_tie_break_deterministic() {
        let entries = vec![
            entry(9, TaskStatusSummary::Working, 0, 1),
            entry(2, TaskStatusSummary::Working, 0, 1),
        ];
        let state = TaskDashboardViewState {
            sort_key: TaskSortKey::WorkerCount,
            direction: TaskSortDirection::Descending,
            ..default()
        };

        assert_eq!(
            state
                .visible_entries(&entries)
                .into_iter()
                .map(|entry| entry.entity.index_u32())
                .collect::<Vec<_>>(),
            vec![2, 9]
        );
    }

    #[test]
    fn every_status_has_the_exact_dashboard_label() {
        assert_eq!(TaskStatusSummary::Working.label(), "Working");
        assert_eq!(
            TaskStatusSummary::PendingEvaluation.label(),
            "Evaluating..."
        );
        assert_eq!(
            TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar).label(),
            "Blocked: No eligible familiar"
        );
        assert_eq!(
            TaskStatusSummary::Blocked(TaskBlockerReason::MissingResourceOrSource).label(),
            "Blocked: Missing resource or source"
        );
        assert_eq!(
            TaskStatusSummary::Blocked(TaskBlockerReason::Unreachable).label(),
            "Blocked: Unreachable"
        );
        assert_eq!(
            TaskStatusSummary::Blocked(TaskBlockerReason::TemporaryContention).label(),
            "Blocked: Waiting for reservation"
        );
        assert_eq!(
            TaskStatusSummary::Blocked(TaskBlockerReason::DependencyWaiting).label(),
            "Blocked: Waiting for dependency"
        );
    }

    #[test]
    fn every_filter_dimension_selects_the_expected_entries() {
        let entries = vec![
            entry_with_work_type(1, WorkType::Chop, TaskStatusSummary::Working, 0, 1),
            entry_with_work_type(
                2,
                WorkType::Mine,
                TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar),
                5,
                0,
            ),
            entry_with_work_type(
                3,
                WorkType::Build,
                TaskStatusSummary::PendingEvaluation,
                10,
                2,
            ),
        ];

        let cases = [
            (
                TaskDashboardViewState {
                    work_type: TaskWorkTypeFilter::Only(WorkType::Mine),
                    ..default()
                },
                vec![2],
            ),
            (
                TaskDashboardViewState {
                    status: TaskStatusFilter::Working,
                    ..default()
                },
                vec![1],
            ),
            (
                TaskDashboardViewState {
                    status: TaskStatusFilter::Blocked,
                    ..default()
                },
                vec![2],
            ),
            (
                TaskDashboardViewState {
                    status: TaskStatusFilter::Pending,
                    ..default()
                },
                vec![3],
            ),
            (
                TaskDashboardViewState {
                    priority: TaskPriorityFilter::Normal,
                    ..default()
                },
                vec![1],
            ),
            (
                TaskDashboardViewState {
                    priority: TaskPriorityFilter::High,
                    ..default()
                },
                vec![2],
            ),
            (
                TaskDashboardViewState {
                    priority: TaskPriorityFilter::Critical,
                    ..default()
                },
                vec![3],
            ),
            (
                TaskDashboardViewState {
                    workers: TaskWorkerFilter::Assigned,
                    ..default()
                },
                vec![1, 3],
            ),
            (
                TaskDashboardViewState {
                    workers: TaskWorkerFilter::Unassigned,
                    ..default()
                },
                vec![2],
            ),
        ];

        for (state, expected) in cases {
            assert_eq!(
                state
                    .visible_entries(&entries)
                    .into_iter()
                    .map(|entry| entry.entity.index_u32())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn every_sort_key_and_direction_has_a_deterministic_order() {
        let entries = vec![
            entry_with_work_type(
                3,
                WorkType::Build,
                TaskStatusSummary::PendingEvaluation,
                10,
                2,
            ),
            entry_with_work_type(1, WorkType::Chop, TaskStatusSummary::Working, 0, 1),
            entry_with_work_type(
                2,
                WorkType::Mine,
                TaskStatusSummary::Blocked(TaskBlockerReason::NoEligibleFamiliar),
                5,
                0,
            ),
        ];
        let cases = [
            (
                TaskSortKey::WorkType,
                TaskSortDirection::Ascending,
                vec![1, 2, 3],
            ),
            (
                TaskSortKey::WorkType,
                TaskSortDirection::Descending,
                vec![3, 2, 1],
            ),
            (
                TaskSortKey::Status,
                TaskSortDirection::Ascending,
                vec![1, 2, 3],
            ),
            (
                TaskSortKey::Status,
                TaskSortDirection::Descending,
                vec![3, 2, 1],
            ),
            (
                TaskSortKey::Priority,
                TaskSortDirection::Ascending,
                vec![1, 2, 3],
            ),
            (
                TaskSortKey::Priority,
                TaskSortDirection::Descending,
                vec![3, 2, 1],
            ),
            (
                TaskSortKey::WorkerCount,
                TaskSortDirection::Ascending,
                vec![2, 1, 3],
            ),
            (
                TaskSortKey::WorkerCount,
                TaskSortDirection::Descending,
                vec![3, 1, 2],
            ),
        ];

        for (sort_key, direction, expected) in cases {
            let state = TaskDashboardViewState {
                sort_key,
                direction,
                ..default()
            };
            assert_eq!(
                state
                    .visible_entries(&entries)
                    .into_iter()
                    .map(|entry| entry.entity.index_u32())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn dashboard_controls_cycle_back_to_their_defaults() {
        let cases = [
            (TaskDashboardControl::WorkTypeFilter, WORK_TYPES.len() + 1),
            (TaskDashboardControl::StatusFilter, 4),
            (TaskDashboardControl::PriorityFilter, 4),
            (TaskDashboardControl::WorkerFilter, 3),
            (TaskDashboardControl::SortKey, 4),
            (TaskDashboardControl::SortDirection, 2),
        ];

        for (control, presses) in cases {
            let mut state = TaskDashboardViewState::default();
            for _ in 0..presses {
                state.apply_control(control);
            }
            assert_eq!(state, TaskDashboardViewState::default());
        }
    }
}
