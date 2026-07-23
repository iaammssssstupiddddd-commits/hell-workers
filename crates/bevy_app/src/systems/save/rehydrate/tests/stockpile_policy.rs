use bevy::prelude::*;
use hw_jobs::Building;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_logistics::transport_request::TransportPriority;
use hw_logistics::{
    BelongsTo, PendingBelongsToBlueprint, Stockpile, StockpileAcceptance, StockpilePolicy,
};
use hw_world::Yard;

use super::rehydrate_stockpile_policies;

fn stockpile(capacity: usize) -> Stockpile {
    Stockpile {
        capacity,
        resource_type: None,
    }
}

#[test]
fn old_save_policy_migration_only_updates_yard_owned_stockpiles() {
    let mut world = World::new();
    let yard = world
        .spawn(Yard {
            min: Vec2::ZERO,
            max: Vec2::splat(10.0),
        })
        .id();
    let tank = world.spawn((Building::default(), stockpile(4))).id();
    let ordinary = world.spawn((stockpile(6), BelongsTo(yard))).id();
    let legacy_tank_companion_without_marker = world.spawn((stockpile(2), BelongsTo(tank))).id();
    let mixer = world
        .spawn((
            Building::default(),
            MudMixerStorage::default(),
            stockpile(3),
        ))
        .id();
    let pending_tank_companion = world
        .spawn((stockpile(2), PendingBelongsToBlueprint(tank)))
        .id();
    let existing_policy = world
        .spawn((
            stockpile(3),
            BelongsTo(yard),
            StockpilePolicy {
                acceptance: StockpileAcceptance::Only(hw_core::logistics::ResourceType::Bone),
                inbound_priority: TransportPriority::Critical,
                target_amount: 99,
                allow_export: false,
            },
        ))
        .id();

    rehydrate_stockpile_policies(&mut world);
    rehydrate_stockpile_policies(&mut world);

    assert_eq!(
        world.get::<StockpilePolicy>(ordinary),
        Some(&StockpilePolicy::for_capacity(6))
    );
    assert!(
        world
            .get::<StockpilePolicy>(legacy_tank_companion_without_marker)
            .is_none()
    );
    assert!(world.get::<StockpilePolicy>(tank).is_none());
    assert!(world.get::<StockpilePolicy>(mixer).is_none());
    assert!(
        world
            .get::<StockpilePolicy>(pending_tank_companion)
            .is_none()
    );
    assert_eq!(
        world.get::<StockpilePolicy>(existing_policy),
        Some(&StockpilePolicy {
            acceptance: StockpileAcceptance::Only(hw_core::logistics::ResourceType::Bone),
            inbound_priority: TransportPriority::Critical,
            target_amount: 3,
            allow_export: false,
        })
    );

    let policy_count = world.query::<&StockpilePolicy>().iter(&world).count();
    assert_eq!(policy_count, 2);
}
