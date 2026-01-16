//! 汎用アニメーション実装
//!
//! パルス、バウンスなどの汎用的なアニメーション効果

use bevy::prelude::*;

/// パルスアニメーション設定
#[derive(Debug, Clone)]
pub struct PulseAnimationConfig {
    /// アニメーション周期（秒）
    pub period: f32,
    /// 最小値（通常は透明度）
    pub min_value: f32,
    /// 最大値（通常は透明度）
    pub max_value: f32,
}

impl Default for PulseAnimationConfig {
    fn default() -> Self {
        Self {
            period: 0.5,
            min_value: 0.8,
            max_value: 1.0,
        }
    }
}

/// パルスアニメーションコンポーネント
#[derive(Component)]
pub struct PulseAnimation {
    /// タイマー
    pub timer: f32,
    /// 設定
    pub config: PulseAnimationConfig,
}

impl Default for PulseAnimation {
    fn default() -> Self {
        Self {
            timer: 0.0,
            config: PulseAnimationConfig::default(),
        }
    }
}

/// パルスアニメーションを更新し、値を返す
pub fn update_pulse_animation(
    time: &Time,
    animation: &mut PulseAnimation,
) -> f32 {
    animation.timer += time.delta_secs();

    // sin波でパルス
    let t = (animation.timer / animation.config.period * std::f32::consts::TAU).sin();
    animation.config.min_value
        + (animation.config.max_value - animation.config.min_value) * (t * 0.5 + 0.5)
}

/// バウンスアニメーション設定
#[derive(Debug, Clone)]
pub struct BounceAnimationConfig {
    /// アニメーション持続時間（秒）
    pub duration: f32,
    /// 最小スケール
    pub min_scale: f32,
    /// 最大スケール
    pub max_scale: f32,
}

impl Default for BounceAnimationConfig {
    fn default() -> Self {
        Self {
            duration: 0.4,
            min_scale: 1.0,
            max_scale: 1.2,
        }
    }
}

/// バウンスアニメーションコンポーネント
#[derive(Component)]
pub struct BounceAnimation {
    /// タイマー
    pub timer: f32,
    /// 設定
    pub config: BounceAnimationConfig,
}

impl Default for BounceAnimation {
    fn default() -> Self {
        Self {
            timer: 0.0,
            config: BounceAnimationConfig::default(),
        }
    }
}

/// バウンスアニメーションを更新し、スケール値を返す
/// アニメーションが完了した場合はNoneを返す
pub fn update_bounce_animation(
    time: &Time,
    animation: &mut BounceAnimation,
) -> Option<f32> {
    animation.timer += time.delta_secs();

    if animation.timer >= animation.config.duration {
        return None;
    }

    // サイン波でスケールを変化させる (min_scale -> max_scale -> min_scale)
    let progress = animation.timer / animation.config.duration;
    let t = (progress * std::f32::consts::PI).sin();
    let scale = animation.config.min_scale
        + (animation.config.max_scale - animation.config.min_scale) * t;

    Some(scale)
}
