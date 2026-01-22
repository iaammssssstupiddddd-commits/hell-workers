//! 建築ビジュアルシステム
//!
//! 設計図（Blueprint）の視覚的フィードバックを管理するモジュール。
//! - 透明度の動的変化
//! - プログレスバー表示
//! - 状態別カラーオーバーレイ
//! - アニメーション効果

mod components;
mod effects;
mod material_display;
mod progress_bar;
mod worker_indicator;

use bevy::prelude::*;

use crate::systems::jobs::Blueprint;
use crate::systems::utils::animations::{PulseAnimation, update_pulse_animation};

// ============================================================================
// Re-exports
// ============================================================================

pub use components::*;
pub use effects::*;
pub use material_display::*;
pub use progress_bar::*;
pub use worker_indicator::*;

// ============================================================================
// 定数
// ============================================================================

/// プログレスバーの幅
pub const PROGRESS_BAR_WIDTH: f32 = 24.0;
/// プログレスバーの高さ
pub const PROGRESS_BAR_HEIGHT: f32 = 4.0;
/// プログレスバーのY軸オフセット（設計図の下）
pub const PROGRESS_BAR_Y_OFFSET: f32 = -18.0;

/// 資材アイコンのオフセット
pub const MATERIAL_ICON_X_OFFSET: f32 = 20.0;
pub const MATERIAL_ICON_Y_OFFSET: f32 = 10.0;
/// カウンターテキストのオフセット（アイコンからの相対）
pub const COUNTER_TEXT_OFFSET: Vec3 = Vec3::new(12.0, 0.0, 0.0);

/// ポップアップの表示時間
pub const POPUP_LIFETIME: f32 = 1.0;
/// 完成テキストの表示時間
pub const COMPLETION_TEXT_LIFETIME: f32 = 1.5;
/// バウンスアニメーションの持続時間
pub const BOUNCE_DURATION: f32 = 0.4;

// ============================================================================
// カラー定義
// ============================================================================

/// 青写真（未着手）の基本色：鮮やかなシアンブルー
pub const COLOR_BLUEPRINT: Color = Color::srgba(0.1, 0.5, 1.0, 1.0);
/// 建築開始後の基本色：本来のテクスチャ色（ホワイトティント）
pub const COLOR_NORMAL: Color = Color::srgba(1.0, 1.0, 1.0, 1.0);

/// プログレスバー背景色
pub const COLOR_PROGRESS_BG: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);
/// プログレスバー前景色（資材搬入中）
pub const COLOR_PROGRESS_MATERIAL: Color = Color::srgba(1.0, 0.7, 0.1, 1.0);
/// プログレスバー前景色（建築中）
pub const COLOR_PROGRESS_BUILD: Color = Color::srgba(0.1, 0.9, 0.3, 1.0);

// ============================================================================
// ユーティリティ関数
// ============================================================================

/// 設計図の状態を計算する
pub fn calculate_blueprint_state(bp: &Blueprint) -> BlueprintState {
    if bp.progress > 0.0 {
        BlueprintState::Building
    } else if bp.materials_complete() {
        BlueprintState::ReadyToBuild
    } else {
        let total_delivered: u32 = bp.delivered_materials.values().sum();
        if total_delivered > 0 {
            BlueprintState::Preparing
        } else {
            BlueprintState::NeedsMaterials
        }
    }
}

/// 設計図の表示設定（色と透明度）を計算する
pub fn calculate_blueprint_visual_props(bp: &Blueprint) -> (Color, f32) {
    let total_required: u32 = bp.required_materials.values().sum();
    let total_delivered: u32 = bp.delivered_materials.values().sum();

    let material_ratio = if total_required > 0 {
        (total_delivered as f32 / total_required as f32).min(1.0)
    } else {
        1.0
    };

    // 透明度: 0.4(ベース) + 0.2(搬入) + 0.4(建築) = 最高 1.0
    let opacity = 0.4 + 0.2 * material_ratio + 0.4 * bp.progress.min(1.0);

    // 色: 未着手(progress=0)は BLUEPRINT、建築開始後は進捗に応じて NORMAL へ
    let color = if bp.progress > 0.0 {
        // 建築が始まったら、本来の色（ホワイトティント）にする
        COLOR_NORMAL
    } else {
        // 未着手時は青写真
        COLOR_BLUEPRINT
    };

    (color, opacity)
}

// ============================================================================
// システム
// ============================================================================

/// BlueprintVisual コンポーネントを持たない Blueprint に自動的に追加する
pub fn attach_blueprint_visual_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, (With<Blueprint>, Without<BlueprintVisual>)>,
) {
    for entity in q_blueprints.iter() {
        commands.entity(entity).insert(BlueprintVisual::default());
    }
}

/// 設計図のビジュアル（色と透明度）を更新する
pub fn update_blueprint_visual_system(
    mut q_blueprints: Query<(&Blueprint, &mut BlueprintVisual, &mut Sprite)>,
) {
    for (bp, mut visual, mut sprite) in q_blueprints.iter_mut() {
        visual.state = calculate_blueprint_state(bp);

        let (color, opacity) = calculate_blueprint_visual_props(bp);
        sprite.color = color.with_alpha(opacity);
    }
}

/// 建築中のパルスアニメーション
pub fn blueprint_pulse_animation_system(
    time: Res<Time>,
    mut q_blueprints: Query<(&mut BlueprintVisual, &mut Sprite)>,
) {
    for (mut visual, mut sprite) in q_blueprints.iter_mut() {
        if visual.state == BlueprintState::Building {
            // パルスアニメーションを初期化（まだない場合）
            if visual.pulse_animation.is_none() {
                visual.pulse_animation = Some(PulseAnimation::default());
            }

            // パルスアニメーションを更新
            if let Some(ref mut pulse) = visual.pulse_animation {
                let pulse_alpha = update_pulse_animation(&time, pulse);
                sprite.color = sprite.color.with_alpha(pulse_alpha);
            }
        } else {
            visual.pulse_animation = None;
        }
    }
}

/// 進捗に応じたスケールアニメーション
pub fn blueprint_scale_animation_system(
    mut q_blueprints: Query<(&Blueprint, &mut Transform), With<BlueprintVisual>>,
) {
    for (bp, mut transform) in q_blueprints.iter_mut() {
        // scale = 0.9 + 0.1 * progress
        let scale = 0.9 + 0.1 * bp.progress.min(1.0);
        transform.scale = Vec3::splat(scale);
    }
}
