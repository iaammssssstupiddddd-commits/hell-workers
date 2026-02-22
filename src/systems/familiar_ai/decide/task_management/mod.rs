//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

mod builders;
mod delegation;
mod policy;
mod task_assigner;
mod task_finder;
mod validator;

pub use delegation::TaskManager;
pub(crate) use delegation::take_reachable_with_cache_calls;
pub(crate) use policy::take_source_selector_scan_snapshot;
pub use task_assigner::AssignTaskContext;
pub use task_assigner::ReservationShadow;
pub type FamiliarTaskAssignmentQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;
pub use task_assigner::assign_task_to_worker;
pub(crate) use task_assigner::{CachedSourceItem, SourceSelectorFrameCache};
pub use task_finder::DelegationCandidate;
pub use task_finder::ScoredDelegationCandidate;
pub use task_finder::collect_scored_candidates;
