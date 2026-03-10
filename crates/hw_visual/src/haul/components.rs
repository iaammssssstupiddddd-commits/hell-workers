//! 運搬関連のコンポーネント定義

use bevy::prelude::*;

#[derive(Component)]
pub struct CarryingItemVisual {
    pub worker: Entity,
}

#[derive(Component)]
pub struct HasCarryingIndicator;

#[derive(Component)]
pub struct DropPopup {
    pub lifetime: f32,
}
