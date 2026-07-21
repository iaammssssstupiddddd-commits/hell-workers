use bevy::prelude::*;
use rand::Rng;

use crate::constants::{FAMILIAR_RECRUIT_FATIGUE_HYSTERESIS, FATIGUE_THRESHOLD, TILE_SIZE};

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

impl FamiliarOperation {
    /// 既存 member のreleaseとtask assignmentに使う疲労閾値。
    ///
    /// recruit用のヒステリシスと混同しないよう、AI consumerはこのAPIを介して
    /// 保存値を取得する。
    pub fn release_fatigue_threshold(&self) -> f32 {
        self.fatigue_threshold
    }

    /// 新規 Soul の recruit に使う疲労閾値。
    ///
    /// 保存値は既存 member の release/task assignment 閾値としてそのまま扱い、
    /// recruit だけを hysteresis 分厳しくする。0（および非有限値）は recruit 無効。
    pub fn recruit_fatigue_threshold(&self) -> Option<f32> {
        let release = self.fatigue_threshold;
        if !release.is_finite() || release <= f32::EPSILON {
            return None;
        }
        Some((release - FAMILIAR_RECRUIT_FATIGUE_HYSTERESIS).max(0.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_release_threshold_disables_recruitment() {
        let operation = FamiliarOperation {
            fatigue_threshold: 0.0,
            ..default()
        };

        assert_eq!(operation.recruit_fatigue_threshold(), None);
    }

    #[test]
    fn positive_release_threshold_has_strictly_lower_recruit_threshold() {
        for release in [f32::EPSILON * 2.0, 0.1, 0.2, 0.8, 1.0] {
            let operation = FamiliarOperation {
                fatigue_threshold: release,
                ..default()
            };
            let recruit = operation.recruit_fatigue_threshold().unwrap();
            assert!(recruit < release);
        }
    }

    #[test]
    fn recruit_threshold_boundaries_are_defined() {
        let expected = [(0.1, 0.0), (0.2, 0.0), (0.8, 0.6), (1.0, 0.8)];

        for (release, expected_recruit) in expected {
            let operation = FamiliarOperation {
                fatigue_threshold: release,
                ..default()
            };
            let actual = operation.recruit_fatigue_threshold().unwrap();
            assert!((actual - expected_recruit).abs() <= f32::EPSILON);
        }
    }

    #[test]
    fn recruit_and_release_use_distinct_thresholds() {
        let operation = FamiliarOperation {
            fatigue_threshold: 0.8,
            ..default()
        };

        assert!((operation.release_fatigue_threshold() - 0.8).abs() <= f32::EPSILON);
        assert!((operation.recruit_fatigue_threshold().unwrap() - 0.6).abs() <= f32::EPSILON);
    }

    #[test]
    fn member_task_assignment_keeps_release_threshold() {
        let operation = FamiliarOperation {
            fatigue_threshold: 0.8,
            ..default()
        };

        assert_eq!(operation.release_fatigue_threshold(), 0.8);
    }

    #[test]
    fn non_finite_release_threshold_disables_recruitment() {
        for release in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let operation = FamiliarOperation {
                fatigue_threshold: release,
                ..default()
            };
            assert_eq!(operation.recruit_fatigue_threshold(), None);
        }
    }
}
