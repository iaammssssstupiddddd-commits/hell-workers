//! タスク実行モジュール — 実装は hw_soul_ai に移設済み。

pub mod common {
    pub use hw_soul_ai::soul_ai::execute::task_execution::common::*;
}
pub mod context {
    pub use hw_familiar_ai::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;
    pub use hw_soul_ai::soul_ai::execute::task_execution::context::{
        ConstructionSiteAccess, DesignationAccess, FamiliarStorageAccess, MutStorageAccess,
        ReservationAccess, StorageAccess, TaskAssignmentQueries, TaskAssignmentReadAccess,
        TaskExecutionContext, TaskQueries, TaskReservationAccess, TaskUnassignQueries,
    };
}
pub mod handler {
    pub use hw_soul_ai::soul_ai::execute::task_execution::handler::{
        TaskHandler, dispatch::execute_haul_with_wheelbarrow, dispatch::run_task_handler,
    };
}
pub mod move_plant {
    pub use hw_soul_ai::soul_ai::execute::task_execution::move_plant::*;
}
pub mod types {
    pub use hw_soul_ai::soul_ai::execute::task_execution::types::*;
}

pub use types::AssignedTask;

pub use hw_soul_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;
pub use hw_soul_ai::soul_ai::execute::task_execution_system::task_execution_system;
