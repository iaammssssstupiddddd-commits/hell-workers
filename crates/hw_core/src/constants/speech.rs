//! 吹き出しシステム (Speech Bubble)

use bevy::prelude::*;

// ----- 吹き出し生存時間 -----
pub const BUBBLE_DURATION_LOW: f32 = 0.8;
pub const BUBBLE_DURATION_NORMAL: f32 = 1.5;
pub const BUBBLE_DURATION_HIGH: f32 = 2.5;
pub const BUBBLE_DURATION_CRITICAL: f32 = 3.5;

/// 吹き出しの話者からのオフセット
pub const SPEECH_BUBBLE_OFFSET: Vec2 = Vec2::new(16.0, 16.0);

// ----- 吹き出しサイズ -----
pub const BUBBLE_SIZE_SOUL_LOW: f32 = 18.0;
pub const BUBBLE_SIZE_SOUL_NORMAL: f32 = 24.0;
pub const BUBBLE_SIZE_SOUL_HIGH: f32 = 28.0;
pub const BUBBLE_SIZE_SOUL_CRITICAL: f32 = 32.0;
pub const BUBBLE_SIZE_FAMILIAR_LOW: f32 = 10.0;
pub const BUBBLE_SIZE_FAMILIAR_NORMAL: f32 = 12.0;
pub const BUBBLE_SIZE_FAMILIAR_HIGH: f32 = 14.0;
pub const BUBBLE_SIZE_FAMILIAR_CRITICAL: f32 = 16.0;

// ----- 吹き出しアニメーション -----
pub const BUBBLE_ANIM_POP_IN_DURATION: f32 = 0.15;
pub const BUBBLE_ANIM_POP_IN_OVERSHOOT: f32 = 1.2;
pub const BUBBLE_ANIM_POP_OUT_DURATION: f32 = 0.3;
pub const BUBBLE_STACK_GAP: f32 = 40.0;
pub const BUBBLE_SHAKE_INTENSITY: f32 = 1.5;
pub const BUBBLE_SHAKE_SPEED: f32 = 40.0;
pub const BUBBLE_BOB_AMPLITUDE: f32 = 3.0;
pub const BUBBLE_BOB_SPEED: f32 = 4.0;

// ----- 吹き出しカラー -----
pub const BUBBLE_COLOR_MOTIVATED: Color = Color::srgba(0.6, 1.0, 0.4, 1.0);
pub const BUBBLE_COLOR_HAPPY: Color = Color::srgba(1.0, 0.7, 0.8, 1.0);
pub const BUBBLE_COLOR_EXHAUSTED: Color = Color::srgba(0.6, 0.6, 0.7, 1.0);
pub const BUBBLE_COLOR_STRESSED: Color = Color::srgba(1.0, 0.4, 0.4, 1.0);
pub const BUBBLE_COLOR_FEARFUL: Color = Color::srgba(0.5, 0.4, 0.7, 1.0);
pub const BUBBLE_COLOR_RELIEVED: Color = Color::srgba(0.4, 0.8, 1.0, 1.0);
pub const BUBBLE_COLOR_RELAXED: Color = Color::srgba(0.4, 1.0, 0.7, 1.0);
pub const BUBBLE_COLOR_FRUSTRATED: Color = Color::srgba(0.7, 0.7, 0.7, 1.0);
pub const BUBBLE_COLOR_UNMOTIVATED: Color = Color::srgba(0.8, 0.8, 0.5, 1.0);
pub const BUBBLE_COLOR_BORED: Color = Color::srgba(0.7, 0.7, 1.0, 0.8);
pub const BUBBLE_COLOR_SLACKING: Color = Color::srgba(0.5, 0.7, 0.5, 1.0);
pub const BUBBLE_COLOR_CHATTING: Color = Color::srgba(1.0, 0.9, 0.6, 1.0);

// ----- 定期セリフシステム (Periodic Emotion) -----
pub const PERIODIC_EMOTION_LOCK_DURATION: f32 = 10.0;
pub const IDLE_EMOTION_MIN_DURATION: f32 = 10.0;
pub const PROBABILITY_PERIODIC_STRESSED: f32 = 0.2;
pub const PROBABILITY_PERIODIC_EXHAUSTED: f32 = 0.2;
pub const PROBABILITY_PERIODIC_UNMOTIVATED: f32 = 0.1;
pub const PROBABILITY_PERIODIC_BORED: f32 = 0.05;
pub const PERIODIC_EMOTION_FRAME_DIVISOR: u32 = 10;
pub const EMOTION_THRESHOLD_STRESSED: f32 = 0.6;
pub const EMOTION_THRESHOLD_EXHAUSTED: f32 = 0.7;
pub const EMOTION_THRESHOLD_UNMOTIVATED: f32 = 0.3;
