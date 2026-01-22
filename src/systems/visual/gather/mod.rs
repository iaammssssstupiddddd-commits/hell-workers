//! 伐採・採掘ビジュアルシステム
//!
//! 伐採（Chop）および採掘（Mine）中のワーカーへの視覚的フィードバックを管理するモジュール。
//! - ワーカー頭上のアイコン表示（斧、ツルハシ）
//! - 対象リソースのハイライト

mod components;
mod resource_highlight;
mod worker_indicator;

use bevy::prelude::Color;

// ============================================================================
// Re-exports
// ============================================================================

pub use resource_highlight::*;
pub use worker_indicator::*;

// ============================================================================
// 定数
// ============================================================================

/// 伐採アイコンの色（緑寄り）
pub const COLOR_CHOP_ICON: Color = Color::srgb(0.4, 0.9, 0.3);
/// 採掘アイコンの色（灰寄り）
pub const COLOR_MINE_ICON: Color = Color::srgb(0.7, 0.7, 0.8);

/// ワーカーアイコンのサイズ
pub const GATHER_ICON_SIZE: f32 = 16.0;
/// ワーカーアイコンのY軸オフセット
pub const GATHER_ICON_Y_OFFSET: f32 = 32.0;
/// bobアニメーションの速度
pub const GATHER_ICON_BOB_SPEED: f32 = 5.0;
/// bobアニメーションの振幅
pub const GATHER_ICON_BOB_AMPLITUDE: f32 = 2.5;
