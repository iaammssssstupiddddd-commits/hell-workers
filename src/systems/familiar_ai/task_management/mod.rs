//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

mod task_finder;
mod task_assigner;
mod delegation;
mod assignment;

pub use delegation::TaskManager;
pub use task_finder::find_unassigned_task_in_area;
pub use task_assigner::AssignTaskContext;
pub use task_assigner::assign_task_to_worker;
pub use task_assigner::prepare_worker_for_task;
pub use task_assigner::ReservationShadow;
