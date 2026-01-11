//! ゲーム状態管理モジュール
//!
//! BevyのStatesシステムを使用してプレイモードを管理します。
//! 段階的移行: Phase1 = BuildMode, Phase2 = ZoneMode, Phase3 = TaskMode

use crate::systems::command::TaskMode;
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use bevy::prelude::*;

/// プレイ中の操作モード
#[derive(States, Default, Debug, Clone, Eq, PartialEq, Hash, Reflect)]
pub enum PlayMode {
    #[default]
    Normal, // 通常操作
    BuildingPlace,   // 建物配置中
    ZonePlace,       // ゾーン配置中
    TaskDesignation, // タスク指定中（伐採/採掘/運搬など）
}

/// 建物配置モード時の詳細コンテキスト
#[derive(Resource, Default)]
pub struct BuildContext(pub Option<BuildingType>);

/// ゾーン配置モード時の詳細コンテキスト
#[derive(Resource, Default)]
pub struct ZoneContext(pub Option<ZoneType>);

/// タスク指定モード時の詳細コンテキスト（既存TaskModeをラップ）
#[derive(Resource, Default)]
pub struct TaskContext(pub TaskMode);

/// PlayMode切替時のログ出力（デバッグ用）
pub fn log_enter_building_mode() {
    info!("STATE: Entered BuildingPlace mode");
}

pub fn log_exit_building_mode() {
    info!("STATE: Exited BuildingPlace mode");
}

pub fn log_enter_zone_mode() {
    info!("STATE: Entered ZonePlace mode");
}

pub fn log_exit_zone_mode() {
    info!("STATE: Exited ZonePlace mode");
}

pub fn log_enter_task_mode() {
    info!("STATE: Entered TaskDesignation mode");
}

pub fn log_exit_task_mode() {
    info!("STATE: Exited TaskDesignation mode");
}
