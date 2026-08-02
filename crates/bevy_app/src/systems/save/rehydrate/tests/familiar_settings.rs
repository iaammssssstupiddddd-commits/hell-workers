use hw_core::familiar::{
    Familiar, FamiliarOperation, FamiliarPolicy, FamiliarWorkPriority, FamiliarWorkRule,
    FamiliarWorkRuleOverride,
};
use hw_core::jobs::WorkType;
use hw_core::relationships::{CommandedBy, Commanding};

use super::*;

#[test]
fn missing_familiar_settings_are_roster_aware_and_idempotent() {
    let mut world = World::new();
    let familiar = world.spawn(Familiar::default()).id();
    let souls = [
        world.spawn(CommandedBy(familiar)).id(),
        world.spawn(CommandedBy(familiar)).id(),
        world.spawn(CommandedBy(familiar)).id(),
    ];
    world.flush();

    rehydrate_familiar_settings(&mut world).unwrap();
    rehydrate_familiar_settings(&mut world).unwrap();

    assert_eq!(
        world
            .get::<FamiliarOperation>(familiar)
            .unwrap()
            .max_controlled_soul,
        souls.len()
    );
    assert_eq!(
        world.get::<FamiliarPolicy>(familiar),
        Some(&FamiliarPolicy::default())
    );
    assert_eq!(
        world
            .get::<Commanding>(familiar)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        souls
    );
}

#[test]
fn saved_operation_is_preserved_and_policy_is_canonicalized() {
    let mut world = World::new();
    let high = FamiliarWorkRule {
        allowed: false,
        priority: FamiliarWorkPriority::High,
    };
    let operation = FamiliarOperation {
        fatigue_threshold: 0.7,
        max_controlled_soul: 5,
    };
    let familiar = world
        .spawn((
            Familiar::default(),
            operation.clone(),
            FamiliarPolicy {
                default_rule: FamiliarWorkRule::default(),
                overrides: vec![
                    FamiliarWorkRuleOverride {
                        work_type: WorkType::Mine,
                        rule: high,
                    },
                    FamiliarWorkRuleOverride {
                        work_type: WorkType::Chop,
                        rule: high,
                    },
                    FamiliarWorkRuleOverride {
                        work_type: WorkType::Mine,
                        rule: FamiliarWorkRule::default(),
                    },
                ],
            },
        ))
        .id();

    rehydrate_familiar_settings(&mut world).unwrap();

    assert_eq!(world.get::<FamiliarOperation>(familiar), Some(&operation));
    assert_eq!(
        world.get::<FamiliarPolicy>(familiar).unwrap().overrides,
        vec![FamiliarWorkRuleOverride {
            work_type: WorkType::Chop,
            rule: high,
        }]
    );
}

#[test]
fn saved_max_below_roster_is_rejected_without_releasing_souls() {
    let mut world = World::new();
    let familiar = world
        .spawn((
            Familiar::default(),
            FamiliarOperation {
                fatigue_threshold: 0.4,
                max_controlled_soul: 1,
            },
            FamiliarPolicy::default(),
        ))
        .id();
    let souls = [
        world.spawn(CommandedBy(familiar)).id(),
        world.spawn(CommandedBy(familiar)).id(),
    ];
    world.flush();

    let error = rehydrate_familiar_settings(&mut world).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("must cover its Commanding roster")
    );
    assert_eq!(
        world
            .get::<Commanding>(familiar)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        souls
    );
    for soul in souls {
        assert_eq!(world.get::<CommandedBy>(soul).unwrap().0, familiar);
    }
}
