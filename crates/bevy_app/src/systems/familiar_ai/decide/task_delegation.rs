//! Familiar AI タスク委譲システムの root adapter（再エクスポート）。
//!
//! 実装は hw_familiar_ai::familiar_ai::decide::task_delegation へ移動済み。
//! bevy_app の FamiliarAiPlugin chain はこのモジュール経由でシステムを参照する。

pub use hw_familiar_ai::familiar_ai::decide::resources::{
    ReachabilityCacheKey, ReachabilityFrameCache,
};
pub use hw_familiar_ai::familiar_ai::decide::task_delegation::{
    FamiliarAiTaskDelegationParams, familiar_task_delegation_system,
};
