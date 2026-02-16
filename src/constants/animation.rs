//! キャラクター移動・アニメーション

// ----- Soul 移動 -----
pub const SOUL_SPEED_BASE: f32 = 60.0;
pub const SOUL_SPEED_MIN: f32 = 20.0;
pub const SOUL_SPEED_MOTIVATION_BONUS: f32 = 40.0;
pub const SOUL_SPEED_LAZINESS_PENALTY: f32 = 30.0;
pub const SOUL_SPEED_EXHAUSTED_MULTIPLIER: f32 = 0.7;
pub const SOUL_SPEED_WHEELBARROW_MULTIPLIER: f32 = 0.7;

// ----- アニメーション (Bob) -----
pub const ANIM_BOB_AMPLITUDE: f32 = 0.05;

// ----- Familiar アニメーション -----
pub const FAMILIAR_MOVE_ANIMATION_FPS: f32 = 5.0;
pub const FAMILIAR_MOVE_ANIMATION_FRAMES: usize = 4;
pub const FAMILIAR_HOVER_SPEED: f32 = 2.8;
pub const FAMILIAR_HOVER_AMPLITUDE_IDLE: f32 = 4.5;
pub const FAMILIAR_HOVER_AMPLITUDE_MOVE: f32 = 3.0;
pub const FAMILIAR_HOVER_TILT_AMPLITUDE: f32 = 0.03;

// ----- Soul 浮遊・表情 -----
pub const SOUL_FLOAT_SWAY_SPEED: f32 = 2.4;
pub const SOUL_FLOAT_SWAY_TILT_IDLE: f32 = 0.06;
pub const SOUL_FLOAT_SWAY_TILT_MOVE: f32 = 0.12;
pub const SOUL_FLOAT_PULSE_SPEED_BASE: f32 = 2.2;
pub const SOUL_FLOAT_PULSE_AMPLITUDE_IDLE: f32 = 0.025;
pub const SOUL_FLOAT_PULSE_AMPLITUDE_MOVE: f32 = 0.04;
pub const SOUL_EVENT_LOCK_TONE_POSITIVE: f32 = 3.0;
pub const SOUL_EVENT_LOCK_TONE_NEGATIVE: f32 = 3.4;
pub const SOUL_EVENT_LOCK_COMPLETED_POSITIVE: f32 = 1.4;
pub const SOUL_EVENT_LOCK_COMPLETED_NEGATIVE: f32 = 1.8;
pub const SOUL_EVENT_LOCK_EXHAUSTED: f32 = 4.0;
pub const SOUL_EVENT_LOCK_GATHERING_OBJECT: f32 = 2.2;

/// 本文用フォントサイズ（ゲーム内ビジュアル用）
pub const FONT_SIZE_BODY: f32 = 16.0;
