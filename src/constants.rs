pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: i32 = 50;
pub const MAP_HEIGHT: i32 = 50;

/// 疲労閾値: この値以上になるとワーカーは休息を取り、タスクを受け付けない
pub const FATIGUE_THRESHOLD: f32 = 0.8;

/// モチベーション閾値: この値以上の場合、ワーカーは次のタスクを探し続ける
pub const MOTIVATION_THRESHOLD: f32 = 0.1;
