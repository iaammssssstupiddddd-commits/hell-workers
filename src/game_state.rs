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
    FloorPlace,      // 床エリア配置中
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

/// companion 配置の対象種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum CompanionPlacementKind {
    BucketStorage,
    SandPile,
}

/// companion 配置の親建物種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum CompanionParentKind {
    Tank,
    MudMixer,
}

/// companion 配置中の状態
#[derive(Debug, Clone, Reflect)]
pub struct CompanionPlacement {
    pub parent_kind: CompanionParentKind,
    pub parent_anchor: (i32, i32),
    pub kind: CompanionPlacementKind,
    pub center: Vec2,
    pub radius: f32,
    pub required: bool,
}

/// companion 配置コンテキスト
#[derive(Resource, Default)]
pub struct CompanionPlacementState(pub Option<CompanionPlacement>);
