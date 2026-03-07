//! ゲーム状態管理モジュール
//!
//! BevyのStatesシステムを使用してプレイモードを管理します。
//! 段階的移行: Phase1 = BuildMode, Phase2 = ZoneMode, Phase3 = TaskMode

use bevy::prelude::*;

/// プレイ中の操作モード
#[derive(States, Default, Debug, Clone, Eq, PartialEq, Hash, Reflect)]
pub enum PlayMode {
    #[default]
    Normal, // 通常操作
    BuildingPlace,   // 建物配置中
    ZonePlace,       // ゾーン配置中
    TaskDesignation, // タスク指定中（伐採/採掘/運搬など）
    FloorPlace,      // 床エリア配置中
    BuildingMove,    // 建物移動モード
}

