//! 使い魔のコンポーネント定義

use bevy::prelude::*;

// コアコンポーネントは hw_core::familiar から再エクスポート
pub use hw_core::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarType,
};
// アニメーション状態は hw_familiar_ai から再エクスポート
pub use hw_familiar_ai::FamiliarAnimation;

/// 使い魔の色割り当てを管理するリソース
#[derive(Resource, Default)]
pub struct FamiliarColorAllocator(pub u32);

/// オーラ演出用コンポーネント
#[derive(Component)]
pub struct FamiliarAura {
    pub pulse_timer: f32,
}

/// オーラのレイヤー種別
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraLayer {
    Border,
    Pulse,
    Outline,
}

/// 使い魔の範囲表示用コンポーネント
#[derive(Component)]
pub struct FamiliarRangeIndicator(pub Entity);
