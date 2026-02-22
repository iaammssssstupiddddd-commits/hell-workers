//! 建物関連の定数

use super::{SOUL_SPEED_BASE, TILE_SIZE};

/// 扉が開くまでの待機時間（秒）
pub const DOOR_OPEN_DURATION_SECS: f32 = 0.5;
/// 扉通過後に自動で閉じるまでの遅延（秒）
pub const DOOR_CLOSE_DELAY_SECS: f32 = 1.0;

const MOVE_COST_STRAIGHT_BASE: f32 = 10.0;

/// 扉を開ける待機時間を、A* コストに換算した追加コスト
pub const DOOR_OPEN_COST: i32 =
    ((DOOR_OPEN_DURATION_SECS / (TILE_SIZE / SOUL_SPEED_BASE)) * MOVE_COST_STRAIGHT_BASE) as i32;
