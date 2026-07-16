//! Familiar の logical root から分離した 2D visual child。
//!
//! Familiar の root Transform は pathfinding / spatial index が読む論理座標である。
//! hover と tilt はこの child だけに適用し、親を animation のために dirty にしない。

use bevy::prelude::*;

/// 2D Familiar sprite の owner link。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarVisualOwner {
    pub owner: Entity,
}

/// child sprite にだけ適用する visual offset。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct FamiliarVisualOffset {
    pub hover_offset: f32,
    pub tilt_radians: f32,
}
