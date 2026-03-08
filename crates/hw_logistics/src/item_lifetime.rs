use bevy::prelude::*;

/// アイテムの寿命を管理するタイマーコンポーネント
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct ItemDespawnTimer(pub Timer);

impl ItemDespawnTimer {
    pub fn new(seconds: f32) -> Self {
        Self(Timer::from_seconds(seconds, TimerMode::Once))
    }
}
