//! Blueprint関連のコンポーネント定義

use bevy::prelude::*;
use std::collections::HashMap;

use crate::systems::logistics::ResourceType;
use crate::systems::utils::animations::{BounceAnimation, PulseAnimation};
use crate::systems::utils::floating_text::FloatingText;

// ============================================================================
// コンポーネント定義
// ============================================================================

/// 設計図の現在の状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlueprintState {
    /// 資材が不足している
    #[default]
    NeedsMaterials,
    /// 資材運搬中（一部搬入済み）
    Preparing,
    /// 資材が揃い、建築可能
    ReadyToBuild,
    /// 建築作業中
    Building,
}

/// 設計図のビジュアル状態を管理するコンポーネント
#[derive(Component, Default)]
pub struct BlueprintVisual {
    /// 現在の状態
    pub state: BlueprintState,
    /// パルスアニメーション（utilを使用）
    pub pulse_animation: Option<PulseAnimation>,
    /// 前フレームの搬入済み資材数（ポップアップ検出用）
    pub last_delivered: HashMap<ResourceType, u32>,
}

/// 資材アイコン表示用コンポーネント
#[derive(Component)]
pub struct MaterialIcon {
    /// 親となる設計図エンティティ
    pub blueprint: Entity,
    /// 表示する資材タイプ
    pub _resource_type: ResourceType,
}

/// 資材カウンター表示用コンポーネント
#[derive(Component)]
pub struct MaterialCounter {
    /// 親となる設計図エンティティ
    pub blueprint: Entity,
    /// 表示する資材タイプ
    pub resource_type: ResourceType,
}

/// 搬入時の「+1」ポップアップ（util::FloatingTextのラッパー）
#[derive(Component)]
pub struct DeliveryPopup {
    /// 内部のFloatingTextコンポーネント
    pub floating_text: FloatingText,
}

/// 完成時のフローティングテキスト（util::FloatingTextのラッパー）
#[derive(Component)]
pub struct CompletionText {
    /// 内部のFloatingTextコンポーネント
    pub floating_text: FloatingText,
}

/// 完成した建物に付与する一時的なバウンス（跳ねる）アニメーション（util::BounceAnimationのラッパー）
#[derive(Component)]
pub struct BuildingBounceEffect {
    /// 内部のBounceAnimationコンポーネント
    pub bounce_animation: BounceAnimation,
}

/// 建築中のワーカー頭上に表示されるハンマーアイコン
#[derive(Component)]
pub struct WorkerHammerIcon {
    pub worker: Entity,
}

/// インジケータが既に付与されていることを示すマーカー
#[derive(Component)]
pub struct HasWorkerIndicator;

/// プログレスバーのマーカーコンポーネント（util::GenericProgressBarのラッパー）
#[derive(Component)]
pub struct ProgressBar {
    /// 親となる設計図エンティティ
    pub blueprint: Entity,
}
