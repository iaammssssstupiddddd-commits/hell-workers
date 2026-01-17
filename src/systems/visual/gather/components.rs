//! 伐採・採掘関連のコンポーネント定義

use bevy::prelude::*;

use crate::systems::utils::animations::PulseAnimation;

/// 伐採・採掘中のワーカー頭上に表示されるアイコン
#[derive(Component)]
pub struct WorkerGatherIcon {
    /// アイコンが紐づくワーカーエンティティ
    pub worker: Entity,
}

/// 伐採・採掘インジケータが既に付与されていることを示すマーカー
#[derive(Component)]
pub struct HasGatherIndicator;

/// リソースのハイライト状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceHighlightState {
    /// 通常状態（ハイライトなし）
    #[default]
    Normal,
    /// 指定済み（パルスアニメーション）
    Designated,
    /// 作業中（透明度変化）
    Working,
}

/// リソース（木、岩等）のビジュアル状態を管理するコンポーネント
#[derive(Component, Default)]
pub struct ResourceVisual {
    /// 現在の状態
    pub state: ResourceHighlightState,
    /// パルスアニメーション（指定済み時）
    pub pulse_animation: Option<PulseAnimation>,
    /// 元のスプライト色（復元用）
    pub original_color: Option<Color>,
}
