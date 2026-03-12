//! 使い魔のタスク管理モジュール（hw_ai への薄いブリッジ）

pub use hw_ai::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow, TaskManager,
    take_reachable_with_cache_calls, take_source_selector_scan_snapshot,
};
