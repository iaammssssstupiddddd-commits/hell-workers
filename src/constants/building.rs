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

/// Room flood-fill の最大タイル数（上限超過時は不成立）
pub const ROOM_MAX_TILES: usize = 400;
/// dirty 収集後に Room 再検出を実行する最小間隔（秒）
pub const ROOM_DETECTION_COOLDOWN_SECS: f32 = 0.5;
/// 既存 Room を再検証する周期（秒）
pub const ROOM_VALIDATION_INTERVAL_SECS: f32 = 2.0;
