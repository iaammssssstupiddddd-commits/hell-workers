use crate::systems::command::TaskMode;
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use bevy::prelude::*;

/// 建物配置モード時の詳細コンテキスト
#[derive(Resource, Default)]
pub struct BuildContext(pub Option<BuildingType>);

/// 建物移動モード時の詳細コンテキスト
#[derive(Resource, Default)]
pub struct MoveContext(pub Option<Entity>);

/// 建物移動モードでの移動先仮決定状態
#[derive(Debug, Clone, Copy)]
pub struct PendingMovePlacement {
    pub building: Entity,
    pub destination_grid: (i32, i32),
}

/// 建物移動モードで companion 指定を待っている状態
#[derive(Resource, Default)]
pub struct MovePlacementState(pub Option<PendingMovePlacement>);

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
}

/// companion 配置の親建物種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum CompanionParentKind {
    Tank,
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
