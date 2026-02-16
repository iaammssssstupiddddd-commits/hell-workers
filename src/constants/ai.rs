//! AI ロジック定数 (fatigue, stress, idle, escape, population)

use super::world::TILE_SIZE;

// ----- 疲労・モチベーション・閾値 -----
pub const FATIGUE_THRESHOLD: f32 = 0.8;
pub const MOTIVATION_THRESHOLD: f32 = 0.3;
pub const FATIGUE_GATHERING_THRESHOLD: f32 = 0.9;
pub const FATIGUE_IDLE_THRESHOLD: f32 = 0.8;

// ----- 疲労 (Fatigue) -----
pub const FATIGUE_WORK_RATE: f32 = 0.01;
pub const FATIGUE_RECOVERY_RATE_COMMANDED: f32 = 0.01;
pub const FATIGUE_RECOVERY_RATE_IDLE: f32 = 0.05;
pub const FATIGUE_MOTIVATION_PENALTY_THRESHOLD: f32 = 0.9;
pub const FATIGUE_MOTIVATION_PENALTY_RATE: f32 = 0.5;
pub const FATIGUE_GAIN_ON_COMPLETION: f32 = 0.1;

// ----- ストレス (Stress) -----
pub const STRESS_WORK_RATE: f32 = 0.005;
pub const STRESS_RECOVERY_RATE_GATHERING: f32 = 0.04;
pub const STRESS_RECOVERY_RATE_IDLE: f32 = 0.02;
pub const STRESS_RECOVERY_THRESHOLD: f32 = 0.7;
pub const STRESS_FREEZE_RECOVERY_THRESHOLD: f32 = 0.9;

// ----- 監視 (Supervision) -----
pub const SUPERVISION_IDLE_MULTIPLIER: f32 = 0.4;
pub const SUPERVISION_STRESS_SCALE: f32 = 0.01;
pub const SUPERVISION_MOTIVATION_SCALE: f32 = 0.4;
pub const SUPERVISION_LAZINESS_SCALE: f32 = 2.5;

// ----- やる気と怠惰 (Motivation & Laziness) -----
pub const MOTIVATION_LOSS_RATE_ACTIVE: f32 = 0.05;
pub const MOTIVATION_LOSS_RATE_IDLE: f32 = 0.1;
pub const LAZINESS_LOSS_RATE_ACTIVE: f32 = 0.1;
pub const LAZINESS_GAIN_RATE_IDLE: f32 = 0.05;
pub const MOTIVATION_BONUS_GATHER: f32 = 0.02;
pub const MOTIVATION_BONUS_HAUL: f32 = 0.01;
pub const MOTIVATION_BONUS_BUILD: f32 = 0.05;
pub const MOTIVATION_PENALTY_CONVERSATION: f32 = 0.02;

// ----- 激励 -----
pub const ENCOURAGEMENT_INTERVAL_MIN: f32 = 5.0;
pub const ENCOURAGEMENT_INTERVAL_MAX: f32 = 10.0;
pub const ENCOURAGEMENT_COOLDOWN: f32 = 30.0;
pub const ENCOURAGEMENT_MOTIVATION_BONUS: f32 = 0.025;
pub const ENCOURAGEMENT_STRESS_PENALTY: f32 = 0.0125;
pub const RECRUIT_MOTIVATION_BONUS: f32 = 0.3;
pub const RECRUIT_STRESS_PENALTY: f32 = 0.1;
pub const EMOJIS_ENCOURAGEMENT: &[&str] = &["👊", "💪", "📢", "⚡", "🔥"];

// ----- 怠惰行動 (Idle Behavior) -----
pub const IDLE_TIME_TO_GATHERING: f32 = 30.0;
pub const LAZINESS_THRESHOLD_HIGH: f32 = 0.8;
pub const LAZINESS_THRESHOLD_MID: f32 = 0.5;
pub const GATHERING_ARRIVAL_RADIUS_BASE: f32 = 5.0;
pub const GATHERING_KEEP_DISTANCE_MIN: f32 = 3.0;
pub const GATHERING_KEEP_DISTANCE_TARGET_MIN: f32 = 3.0;
pub const GATHERING_KEEP_DISTANCE_TARGET_MAX: f32 = 4.5;
pub const GATHERING_BEHAVIOR_DURATION_MIN: f32 = 10.0;
pub const GATHERING_BEHAVIOR_DURATION_MAX: f32 = 20.0;
pub const IDLE_DURATION_SLEEP_MIN: f32 = 5.0;
pub const IDLE_DURATION_SLEEP_MAX: f32 = 10.0;
pub const IDLE_DURATION_SIT_MIN: f32 = 3.0;
pub const IDLE_DURATION_SIT_MAX: f32 = 6.0;
pub const IDLE_DURATION_WANDER_MIN: f32 = 2.0;
pub const IDLE_DURATION_WANDER_MAX: f32 = 4.0;

// ----- 休憩所 -----
pub const REST_AREA_CAPACITY: usize = 5;
pub const REST_AREA_DREAM_RATE: f32 = 0.12;
pub const REST_AREA_RECRUIT_COOLDOWN_SECS: f32 = 15.0;
pub const REST_AREA_FATIGUE_RECOVERY_RATE: f32 = 0.08;
pub const REST_AREA_STRESS_RECOVERY_RATE: f32 = 0.03;
pub const REST_AREA_RESTING_DURATION: f32 = 180.0;

// ----- 逃走システム (Escape) -----
pub const ESCAPE_TRIGGER_DISTANCE_MULTIPLIER: f32 = 1.5;
pub const ESCAPE_SAFE_DISTANCE_MULTIPLIER: f32 = 2.0;
pub const ESCAPE_SPEED_MULTIPLIER: f32 = 1.3;
pub const ESCAPE_STRESS_THRESHOLD: f32 = 0.3;
pub const ESCAPE_PROXIMITY_STRESS_RATE: f32 = 0.005;
pub const ESCAPE_GATHERING_JOIN_RADIUS: f32 = TILE_SIZE * 7.5;
pub const ESCAPE_DETECTION_INTERVAL: f32 = 0.5;
pub const ESCAPE_BEHAVIOR_INTERVAL: f32 = 0.5;

// ----- Soul 供給/脱走 (Population & Drift) -----
pub const SOUL_SPAWN_INITIAL: u32 = 10;
pub const SOUL_SPAWN_INTERVAL: f32 = 60.0;
pub const SOUL_SPAWN_COUNT_MIN: u32 = 1;
pub const SOUL_SPAWN_COUNT_MAX: u32 = 2;
pub const SOUL_POPULATION_BASE_CAP: u32 = 10;
pub const SOUL_POPULATION_PER_REST_AREA: u32 = 5;
pub const SOUL_ESCAPE_UNMANAGED_TIME: f32 = 120.0;
pub const SOUL_ESCAPE_CHECK_INTERVAL: f32 = 10.0;
pub const SOUL_ESCAPE_CHANCE_PER_CHECK: f64 = 0.3;
pub const SOUL_ESCAPE_GLOBAL_COOLDOWN: f32 = 30.0;
pub const DRIFT_WANDER_DURATION_MIN: f32 = 5.0;
pub const DRIFT_WANDER_DURATION_MAX: f32 = 10.0;
pub const DRIFT_MOVE_TILES_MIN: i32 = 3;
pub const DRIFT_MOVE_TILES_MAX: i32 = 6;
pub const DRIFT_LATERAL_OFFSET_MAX: i32 = 2;
pub const SOUL_DESPAWN_EDGE_MARGIN_TILES: i32 = 2;

// ----- スケーラビリティ最適化 -----
pub const FAMILIAR_TASK_DELEGATION_INTERVAL: f32 = 0.5;
pub const RESERVATION_SYNC_INTERVAL: f32 = 0.2;
pub const SPATIAL_GRID_SYNC_INTERVAL: f32 = 0.15;
