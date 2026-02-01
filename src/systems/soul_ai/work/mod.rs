//! 作業管理モジュール
//!
//! 魂へのタスク解除や自動運搬ロジックを管理します。

use bevy::prelude::*;

pub mod auto_build;
pub mod auto_haul;
pub mod auto_refine;
pub mod cleanup;
pub mod helpers;

// 外部からの参照のために再公開
pub use auto_haul::task_area_auto_haul_system;
pub use auto_haul::bucket_auto_haul_system;
pub use auto_haul::tank_water_request_system;
pub use helpers::unassign_task;

/// 実行頻度を制御するためのカウンター
#[derive(Resource, Default)]
pub struct AutoHaulCounter;
