//! 使い魔のアニメーション状態コンポーネント

use bevy::prelude::*;

/// 使い魔のアニメーション状態
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct FamiliarAnimation {
    pub timer: f32,
    pub frame: usize,
    pub is_moving: bool,
    pub facing_right: bool,
    pub hover_timer: f32,
    pub hover_offset: f32,
}
