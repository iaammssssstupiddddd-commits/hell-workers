//! Dream システム

/// VividDream の蓄積レート (ポイント/秒)
pub const DREAM_RATE_VIVID: f32 = 0.15;
/// NormalDream の蓄積レート (ポイント/秒)
pub const DREAM_RATE_NORMAL: f32 = 0.1;
/// 悪夢判定のストレス閾値（これ以上で NightTerror）
pub const DREAM_NIGHTMARE_STRESS_THRESHOLD: f32 = 0.7;
/// VividDream 判定のストレス閾値（これ以下＋集会中で VividDream）
pub const DREAM_VIVID_STRESS_THRESHOLD: f32 = 0.3;
