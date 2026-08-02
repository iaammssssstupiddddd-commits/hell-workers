use bevy::prelude::*;
use rand::Rng;

use crate::constants::{FAMILIAR_RECRUIT_FATIGUE_HYSTERESIS, FATIGUE_THRESHOLD, TILE_SIZE};
use crate::jobs::WorkType;

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
#[derive(Component, Reflect, Debug, Clone, PartialEq)]
#[reflect(Component)]
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

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FamiliarWorkPriority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarWorkRule {
    pub allowed: bool,
    pub priority: FamiliarWorkPriority,
}

impl Default for FamiliarWorkRule {
    fn default() -> Self {
        Self {
            allowed: true,
            priority: FamiliarWorkPriority::Normal,
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarWorkRuleOverride {
    pub work_type: WorkType,
    pub rule: FamiliarWorkRule,
}

#[derive(Component, Reflect, Debug, Clone, PartialEq, Eq, Default)]
#[reflect(Component)]
pub struct FamiliarPolicy {
    pub default_rule: FamiliarWorkRule,
    pub overrides: Vec<FamiliarWorkRuleOverride>,
}

impl FamiliarPolicy {
    #[must_use]
    pub fn rule_for(&self, work_type: WorkType) -> FamiliarWorkRule {
        self.overrides
            .iter()
            .rev()
            .find(|entry| entry.work_type == work_type)
            .map_or(self.default_rule, |entry| entry.rule)
    }

    pub fn set_rule(&mut self, work_type: WorkType, rule: FamiliarWorkRule) {
        self.normalize();
        self.overrides.retain(|entry| entry.work_type != work_type);
        if rule != self.default_rule {
            self.overrides
                .push(FamiliarWorkRuleOverride { work_type, rule });
            self.overrides
                .sort_unstable_by_key(|entry| entry.work_type.stable_index());
        }
    }

    pub fn set_all_allowed(&mut self, allowed: bool) {
        let priorities = WorkType::ALL.map(|work_type| self.rule_for(work_type).priority);
        self.default_rule.allowed = allowed;
        self.overrides = WorkType::ALL
            .into_iter()
            .zip(priorities)
            .filter_map(|(work_type, priority)| {
                let rule = FamiliarWorkRule { allowed, priority };
                (rule != self.default_rule).then_some(FamiliarWorkRuleOverride { work_type, rule })
            })
            .collect();
    }

    pub fn normalize(&mut self) {
        let mut last_rules = [None; WorkType::COUNT];
        for entry in &self.overrides {
            last_rules[entry.work_type.stable_index()] = Some(entry.rule);
        }

        self.overrides = WorkType::ALL
            .into_iter()
            .filter_map(|work_type| {
                last_rules[work_type.stable_index()]
                    .filter(|rule| *rule != self.default_rule)
                    .map(|rule| FamiliarWorkRuleOverride { work_type, rule })
            })
            .collect();
    }

    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.normalize();
        self
    }

    #[must_use]
    pub fn all_work_disabled(&self) -> bool {
        WorkType::ALL
            .into_iter()
            .all(|work_type| !self.rule_for(work_type).allowed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarSettingsPatch {
    AdjustFatigueThreshold {
        steps: i8,
    },
    AdjustMaxControlledSoul {
        delta: i8,
    },
    SetWorkAllowed {
        work_type: WorkType,
        allowed: bool,
    },
    SetWorkPriority {
        work_type: WorkType,
        priority: FamiliarWorkPriority,
    },
    SetAllWorkAllowed {
        allowed: bool,
    },
}

impl FamiliarSettingsPatch {
    pub fn apply(self, operation: &mut FamiliarOperation, policy: &mut FamiliarPolicy) {
        match self {
            Self::AdjustFatigueThreshold { steps } => {
                let next = (operation.fatigue_threshold + f32::from(steps) * 0.1).clamp(0.0, 1.0);
                operation.fatigue_threshold = (next * 10.0).round() / 10.0;
            }
            Self::AdjustMaxControlledSoul { delta } => {
                let next = if delta.is_negative() {
                    operation
                        .max_controlled_soul
                        .saturating_sub(usize::from(delta.unsigned_abs()))
                } else {
                    operation
                        .max_controlled_soul
                        .saturating_add(usize::from(delta as u8))
                };
                operation.max_controlled_soul = next.clamp(1, 8);
            }
            Self::SetWorkAllowed { work_type, allowed } => {
                let mut rule = policy.rule_for(work_type);
                rule.allowed = allowed;
                policy.set_rule(work_type, rule);
            }
            Self::SetWorkPriority {
                work_type,
                priority,
            } => {
                let mut rule = policy.rule_for(work_type);
                rule.priority = priority;
                policy.set_rule(work_type, rule);
            }
            Self::SetAllWorkAllowed { allowed } => policy.set_all_allowed(allowed),
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

    #[test]
    fn familiar_policy_defaults_to_all_allowed_normal() {
        let policy = FamiliarPolicy::default();

        for work_type in WorkType::ALL {
            assert_eq!(policy.rule_for(work_type), FamiliarWorkRule::default());
        }
        assert!(!policy.all_work_disabled());
        assert!(policy.overrides.is_empty());
    }

    #[test]
    fn familiar_policy_normalization_is_last_wins_stable_and_sparse() {
        let low = FamiliarWorkRule {
            allowed: true,
            priority: FamiliarWorkPriority::Low,
        };
        let disabled = FamiliarWorkRule {
            allowed: false,
            priority: FamiliarWorkPriority::High,
        };
        let mut policy = FamiliarPolicy {
            default_rule: FamiliarWorkRule::default(),
            overrides: vec![
                FamiliarWorkRuleOverride {
                    work_type: WorkType::Mine,
                    rule: low,
                },
                FamiliarWorkRuleOverride {
                    work_type: WorkType::Chop,
                    rule: disabled,
                },
                FamiliarWorkRuleOverride {
                    work_type: WorkType::Mine,
                    rule: FamiliarWorkRule::default(),
                },
            ],
        };

        policy.normalize();

        assert_eq!(
            policy.overrides,
            vec![FamiliarWorkRuleOverride {
                work_type: WorkType::Chop,
                rule: disabled,
            }]
        );
        assert_eq!(policy.rule_for(WorkType::Mine), FamiliarWorkRule::default());
    }

    #[test]
    fn set_all_allowed_preserves_effective_priorities_and_future_default() {
        let mut policy = FamiliarPolicy::default();
        policy.set_rule(
            WorkType::Haul,
            FamiliarWorkRule {
                allowed: true,
                priority: FamiliarWorkPriority::High,
            },
        );
        policy.set_rule(
            WorkType::Build,
            FamiliarWorkRule {
                allowed: false,
                priority: FamiliarWorkPriority::Low,
            },
        );

        policy.set_all_allowed(false);

        assert!(policy.all_work_disabled());
        assert!(!policy.default_rule.allowed);
        assert_eq!(
            policy.rule_for(WorkType::Haul).priority,
            FamiliarWorkPriority::High
        );
        assert_eq!(
            policy.rule_for(WorkType::Build).priority,
            FamiliarWorkPriority::Low
        );

        policy.set_all_allowed(true);
        assert!(!policy.all_work_disabled());
        assert!(policy.default_rule.allowed);
        assert_eq!(
            policy.rule_for(WorkType::Haul).priority,
            FamiliarWorkPriority::High
        );
        assert_eq!(
            policy.rule_for(WorkType::Build).priority,
            FamiliarWorkPriority::Low
        );
    }

    #[test]
    fn settings_patch_clamps_and_preserves_disabled_priority() {
        let mut operation = FamiliarOperation {
            fatigue_threshold: 0.95,
            max_controlled_soul: 8,
        };
        let mut policy = FamiliarPolicy::default();

        FamiliarSettingsPatch::AdjustFatigueThreshold { steps: 1 }
            .apply(&mut operation, &mut policy);
        FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: 1 }
            .apply(&mut operation, &mut policy);
        FamiliarSettingsPatch::SetWorkPriority {
            work_type: WorkType::Chop,
            priority: FamiliarWorkPriority::High,
        }
        .apply(&mut operation, &mut policy);
        FamiliarSettingsPatch::SetWorkAllowed {
            work_type: WorkType::Chop,
            allowed: false,
        }
        .apply(&mut operation, &mut policy);
        FamiliarSettingsPatch::SetWorkAllowed {
            work_type: WorkType::Chop,
            allowed: true,
        }
        .apply(&mut operation, &mut policy);

        assert_eq!(operation.fatigue_threshold, 1.0);
        assert_eq!(operation.max_controlled_soul, 8);
        assert_eq!(
            policy.rule_for(WorkType::Chop),
            FamiliarWorkRule {
                allowed: true,
                priority: FamiliarWorkPriority::High,
            }
        );
    }
}
