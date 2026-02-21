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
pub const DREAM_POPUP_INTERVAL: f32 = 0.5;
pub const DREAM_POPUP_THRESHOLD: f32 = 0.5;
pub const DREAM_POPUP_LIFETIME: f32 = 0.8;
pub const DREAM_POPUP_VELOCITY_Y: f32 = 18.0;
pub const DREAM_POPUP_FONT_SIZE: f32 = 11.0;
pub const DREAM_POPUP_OFFSET_Y: f32 = 18.0;

// Dream pool UI pulse
pub const DREAM_UI_PULSE_DURATION: f32 = 0.35;
pub const DREAM_UI_PULSE_TRIGGER_DELTA: f32 = 0.05;
pub const DREAM_UI_PULSE_BRIGHTNESS: f32 = 0.8;

// UI Particle base (Physics V2)
pub const DREAM_UI_PARTICLE_SIZE: f32 = 14.14; // sqrt(200) instead of sqrt(400) for half area
pub const DREAM_UI_BUOYANCY: f32 = 110.0;
pub const DREAM_UI_BASE_ATTRACTION: f32 = 50.0;
pub const DREAM_UI_BASE_MASS_OFFSET: f32 = 1.0; // 質量にプラスする基本値 (最低限の移動とサイズを保証)
pub const DREAM_UI_VORTEX_STRENGTH: f32 = 5.0; // Keep proportional ratio to attraction
pub const DREAM_UI_DRAG: f32 = 0.85;           // Drag remains the same as acceleration increased
pub const DREAM_UI_STRONG_DRAG: f32 = 0.6;     // アイコン近接時の強いブレーキ
pub const DREAM_UI_NOISE_STRENGTH: f32 = 120.0;
pub const DREAM_UI_NOISE_INTERVAL: f32 = 0.3;
pub const DREAM_UI_BOUNDARY_MARGIN: f32 = 30.0;
pub const DREAM_UI_BOUNDARY_PUSH: f32 = 300.0;
pub const DREAM_UI_BOUNDARY_DAMPING: f32 = 0.1; // 画面端到達時の速度減衰係数
pub const DREAM_UI_MIN_SPEED: f32 = 40.0;       // スタック防止のための最低保証速度
pub const DREAM_UI_FAILSAFE_MARGIN: f32 = 100.0;// 万一画面外へ飛んだ際のフェイルセーフ判定マージン
pub const DREAM_UI_ARRIVAL_RADIUS: f32 = 40.0;

// Size Dynamics
pub const DREAM_UI_SQUASH_MAX_SPEED: f32 = 150.0;
pub const DREAM_UI_SQUASH_MAX_RATIO: f32 = 1.5;

// Merge
pub const DREAM_UI_MERGE_RADIUS: f32 = 30.0;
pub const DREAM_UI_MERGE_MAX_COUNT: u8 = 8;
pub const DREAM_UI_MERGE_MAX_MASS: f32 = 12.0;
pub const DREAM_UI_MERGE_DURATION: f32 = 0.25;
pub const DREAM_UI_MERGE_PULL_FORCE: f32 = 15.0; // 合体時に引き合うバネの力

// Trail
pub const DREAM_UI_TRAIL_INTERVAL: f32 = 0.12;
pub const DREAM_UI_TRAIL_LIFETIME: f32 = 0.15;
pub const DREAM_UI_TRAIL_SIZE_RATIO: f32 = 0.5;
pub const DREAM_UI_TRAIL_ALPHA: f32 = 0.2;

// Icon absorb
pub const DREAM_ICON_ABSORB_DURATION: f32 = 0.25;
pub const DREAM_ICON_BASE_SIZE: f32 = 16.0;
pub const DREAM_ICON_PULSE_SIZE: f32 = 20.0;

// Bubble drift (漂い揺らぎ) removed in V2

// Dream Tree Planting
pub const DREAM_TREE_SPAWN_RATE_PER_TILE: f32 = 0.25;
pub const DREAM_TREE_COST_PER_TREE: f32 = 20.0;
pub const DREAM_TREE_MAX_PER_CAST: u32 = 20;
pub const DREAM_TREE_GLOBAL_CAP: u32 = 300;
