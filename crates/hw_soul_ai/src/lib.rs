pub mod soul_ai;
pub use soul_ai::SoulAiCorePlugin;
pub use soul_ai::decide::drifting::{DriftingDecisionTimer, drifting_decision_system};
pub use soul_ai::helpers::work::{cleanup_task_assignment, is_soul_available_for_work, unassign_task};
