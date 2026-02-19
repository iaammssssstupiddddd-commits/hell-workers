//! Dream システム

/// VividDream の蓄積レート (ポイント/秒)
pub const DREAM_RATE_VIVID: f32 = 0.15;
/// NormalDream の蓄積レート (ポイント/秒)
pub const DREAM_RATE_NORMAL: f32 = 0.1;
/// 悪夢判定のストレス閾値（これ以上で NightTerror）
pub const DREAM_NIGHTMARE_STRESS_THRESHOLD: f32 = 0.7;
/// VividDream 判定のストレス閾値（これ以下＋集会中で VividDream）
pub const DREAM_VIVID_STRESS_THRESHOLD: f32 = 0.3;

// Dream particle visual
pub const DREAM_PARTICLE_MAX_PER_SOUL: u8 = 5;
pub const DREAM_PARTICLE_LIFETIME_VIVID: f32 = 1.0;
pub const DREAM_PARTICLE_LIFETIME_NORMAL: f32 = 0.9;
pub const DREAM_PARTICLE_LIFETIME_TERROR: f32 = 1.2;
pub const DREAM_PARTICLE_INTERVAL_VIVID: f32 = 0.16;
pub const DREAM_PARTICLE_INTERVAL_NORMAL: f32 = 0.22;
pub const DREAM_PARTICLE_INTERVAL_TERROR: f32 = 0.28;
pub const DREAM_PARTICLE_SIZE_MIN: f32 = 5.0;
pub const DREAM_PARTICLE_SIZE_MAX: f32 = 9.0;
pub const DREAM_PARTICLE_SPAWN_OFFSET: f32 = 8.0;
pub const DREAM_PARTICLE_SWAY_VIVID: f32 = 9.0;
pub const DREAM_PARTICLE_SWAY_TERROR: f32 = 5.0;

// Dream popup visual
pub const DREAM_POPUP_THRESHOLD: f32 = 0.08;
pub const DREAM_POPUP_LIFETIME: f32 = 0.8;
pub const DREAM_POPUP_VELOCITY_Y: f32 = 18.0;
pub const DREAM_POPUP_FONT_SIZE: f32 = 11.0;
pub const DREAM_POPUP_OFFSET_Y: f32 = 18.0;

// Dream pool UI pulse
pub const DREAM_UI_PULSE_DURATION: f32 = 0.35;
pub const DREAM_UI_PULSE_TRIGGER_DELTA: f32 = 0.05;
pub const DREAM_UI_PULSE_BRIGHTNESS: f32 = 0.8;
