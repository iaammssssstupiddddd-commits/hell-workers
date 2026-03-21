use bevy::prelude::Reflect;

/// グリッド座標（マップ上の整数タイル位置 (x, y)）
pub type GridPos = (i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DoorState {
    Open,
    Closed,
    Locked,
}
