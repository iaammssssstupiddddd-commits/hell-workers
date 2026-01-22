//! 運搬関連のコンポーネント定義

use bevy::prelude::*;

/// ワーカーが運搬中のアイテムを表すビジュアルアイコン
#[derive(Component)]
pub struct CarryingItemVisual {
    /// ビジュアルが紐づくワーカーエンティティ
    pub worker: Entity,
}

/// 運搬アイコンが既に付与されていることを示すマーカー
#[derive(Component)]
pub struct HasCarryingIndicator;

/// ドロップ時のポップアップ
#[derive(Component)]
pub struct DropPopup {
    /// 残り表示時間
    pub lifetime: f32,
}
