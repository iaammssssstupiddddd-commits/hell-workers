//! Soul 人口管理リソース。
//!
//! スポーン・脱走クールダウンの状態を保持する。
//! `bevy_app` 側の spawn システムが更新し、AI 意思決定システムが参照する。

use crate::constants::{
    SOUL_ESCAPE_GLOBAL_COOLDOWN, SOUL_POPULATION_BASE_CAP, SOUL_SPAWN_INTERVAL,
};
use bevy::prelude::*;

/// Soul の人口管理状態
#[derive(Resource)]
pub struct PopulationManager {
    pub current_count: u32,
    pub population_cap: u32,
    pub total_spawned: u32,
    pub total_escaped: u32,
    pub escape_cooldown_remaining: f32,
    pub spawn_timer: Timer,
}

impl Default for PopulationManager {
    fn default() -> Self {
        Self {
            current_count: 0,
            population_cap: SOUL_POPULATION_BASE_CAP,
            total_spawned: 0,
            total_escaped: 0,
            escape_cooldown_remaining: 0.0,
            spawn_timer: Timer::from_seconds(SOUL_SPAWN_INTERVAL, TimerMode::Repeating),
        }
    }
}

impl PopulationManager {
    pub fn can_start_escape(&self) -> bool {
        self.escape_cooldown_remaining <= f32::EPSILON
    }

    pub fn start_escape_cooldown(&mut self) {
        self.escape_cooldown_remaining = SOUL_ESCAPE_GLOBAL_COOLDOWN;
    }
}
