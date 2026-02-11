//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

mod builders;
mod delegation;
mod policy;
mod validator;
mod task_assigner;
mod task_finder;

pub use delegation::TaskManager;
pub use task_assigner::AssignTaskContext;
pub use task_assigner::ReservationShadow;
pub use task_assigner::assign_task_to_worker;
pub use task_finder::find_unassigned_task_in_area;
