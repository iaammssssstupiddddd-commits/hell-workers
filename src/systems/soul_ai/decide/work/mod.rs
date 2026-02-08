//! 作業管理モジュール
//!
//! 魂へのタスク解除や自動運搬ロジックを管理します。

use bevy::prelude::*;

pub mod auto_build;
pub mod auto_haul;
pub mod auto_refine;

// 外部からの参照のために再公開
pub use crate::systems::soul_ai::helpers::work::unassign_task;
pub use auto_haul::tank_water_request_system;
pub use auto_haul::task_area_auto_haul_system;

/// 実行頻度を制御するためのカウンター
#[derive(Resource, Default)]
pub struct AutoHaulCounter;
