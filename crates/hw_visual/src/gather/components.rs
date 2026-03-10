//! 伐採・採掘関連のコンポーネント定義

use bevy::prelude::*;

use crate::animations::PulseAnimation;

/// 伐採・採掘中のワーカー頭上に表示されるアイコン
#[derive(Component)]
pub struct WorkerGatherIcon {
    pub worker: Entity,
}

/// 伐採・採掘インジケータが既に付与されていることを示すマーカー
#[derive(Component)]
pub struct HasGatherIndicator;

/// リソースのハイライト状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceHighlightState {
    #[default]
    Normal,
    Designated,
    Working,
}

/// リソース（木、岩等）のビジュアル状態を管理するコンポーネント
#[derive(Component, Default)]
pub struct ResourceVisual {
    pub state: ResourceHighlightState,
    pub pulse_animation: Option<PulseAnimation>,
    pub original_color: Option<Color>,
}
