//! GeneratePower タスク型定義

use bevy::prelude::*;

/// GeneratePower タスクの実行フェーズ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum GeneratePowerPhase {
    /// SoulSpaTile へ移動中
    #[default]
    GoingToTile,
    /// タイル到着済み・発電中
    Generating,
}

/// GeneratePower タスクのデータ
#[derive(Debug, Clone, Reflect)]
pub struct GeneratePowerData {
    /// 目標の SoulSpaTile エンティティ
    pub tile: Entity,
    /// 目標タイルのワールド座標
    pub tile_pos: Vec2,
    /// 現在のフェーズ
    pub phase: GeneratePowerPhase,
}
