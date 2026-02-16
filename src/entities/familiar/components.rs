//! 使い魔のコンポーネント定義

use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;

/// 使い魔の名前リスト（10候補）
const FAMILIAR_NAMES: [&str; 10] = [
    "Skrix",   // 小鬼
    "Grubble", // 這いずり
    "Snitch",  // 密告者
    "Grimkin", // 陰気な小者
    "Blotch",  // シミ
    "Scraps",  // くず拾い
    "Nub",     // ちび
    "Whimper", // めそめそ
    "Cringe",  // へつらい
    "Slunk",   // こそこそ
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

/// 使い魔の色割り当てを管理するリソース
#[derive(Resource, Default)]
pub struct FamiliarColorAllocator(pub u32);

/// オーラ演出用コンポーネント
#[derive(Component)]
pub struct FamiliarAura {
    pub pulse_timer: f32,
}

/// オーラのレイヤー種別
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraLayer {
    Border,
    Pulse,
    Outline,
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

/// 使い魔の範囲表示用コンポーネント
#[derive(Component)]
pub struct FamiliarRangeIndicator(pub Entity);
