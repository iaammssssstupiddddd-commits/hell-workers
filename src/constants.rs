pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: i32 = 50;
pub const MAP_HEIGHT: i32 = 50;

/// 使い魔ごとの疲労閾値のデフォルト値
/// 使い魔はこの値をUIで個別に調整可能
pub const FATIGUE_THRESHOLD: f32 = 0.8;

/// モチベーション閾値: この値以上の場合、ワーカーは次のタスクを探し続ける
pub const MOTIVATION_THRESHOLD: f32 = 0.1;

/// 集会閾値: 疲労がこの値を超えると強制的に集会へ向かう（グローバル）
pub const FATIGUE_GATHERING_THRESHOLD: f32 = 0.9;

/// 怠惰行動閾値: 疲労がこの値以上になると怠惰行動を開始（グローバル）
/// 使い魔ごとの閾値とは独立して機能する
pub const FATIGUE_IDLE_THRESHOLD: f32 = 0.8;
