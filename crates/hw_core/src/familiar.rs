use bevy::prelude::*;
use rand::Rng;

use crate::constants::{FATIGUE_THRESHOLD, TILE_SIZE};

/// 使い魔の名前リスト
const FAMILIAR_NAMES: [&str; 10] = [
    "Skrix", "Grubble", "Snitch", "Grimkin", "Blotch", "Scraps", "Nub", "Whimper", "Cringe",
    "Slunk",
];

/// 使い魔のコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Familiar {
    pub familiar_type: FamiliarType,
    pub command_radius: f32,
    pub efficiency: f32,
    pub name: String,
    pub color_index: u32,
}

impl Default for Familiar {
    fn default() -> Self {
        Self {
            familiar_type: FamiliarType::default(),
            command_radius: TILE_SIZE * 7.0,
            efficiency: 0.5,
            name: String::new(),
            color_index: 0,
        }
    }
}

impl Familiar {
    pub fn new(familiar_type: FamiliarType, color_index: u32) -> Self {
        let (command_radius, efficiency) = match familiar_type {
            FamiliarType::Imp => (TILE_SIZE * 7.0, 0.5),
        };
        let mut rng = rand::thread_rng();
        let name = FAMILIAR_NAMES[rng.gen_range(0..FAMILIAR_NAMES.len())].to_string();
        Self {
            familiar_type,
            command_radius,
            efficiency,
            name,
            color_index,
        }
    }
}

/// 使い魔の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum FamiliarType {
    #[default]
    Imp,
}

/// 使い魔への指示
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum FamiliarCommand {
    #[default]
    Idle,
    GatherResources,
    Patrol,
}

/// 現在のアクティブな指示
#[derive(Component, Default)]
pub struct ActiveCommand {
    pub command: FamiliarCommand,
}

/// 使い魔の運用設定
#[derive(Component, Debug, Clone)]
pub struct FamiliarOperation {
    pub fatigue_threshold: f32,
    pub max_controlled_soul: usize,
}

impl Default for FamiliarOperation {
    fn default() -> Self {
        Self {
            fatigue_threshold: FATIGUE_THRESHOLD,
            max_controlled_soul: 2,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
#[derive(Default)]
pub enum FamiliarAiState {
    #[default]
    Idle,
    SearchingTask,
    Scouting {
        target_soul: Entity,
    },
    Supervising {
        target: Option<Entity>,
        timer: f32,
    },
}
