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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskModeZoneType {
    Stockpile,
    Yard,
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub enum TaskMode {
    #[default]
    None,
    DesignateChop(Option<Vec2>),
    DesignateMine(Option<Vec2>),
    DesignateHaul(Option<Vec2>),
    CancelDesignation(Option<Vec2>),
    SelectBuildTarget,
    AreaSelection(Option<Vec2>),
    AssignTask(Option<Vec2>),
    ZonePlacement(TaskModeZoneType, Option<Vec2>),
    ZoneRemoval(TaskModeZoneType, Option<Vec2>),
    FloorPlace(Option<Vec2>),
    WallPlace(Option<Vec2>),
    DreamPlanting(Option<Vec2>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeSpeed {
    Paused,
    Normal,
    Fast,
    Super,
}

