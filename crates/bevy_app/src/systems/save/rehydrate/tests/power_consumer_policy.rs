use super::*;
use hw_energy::{PowerConsumer, PowerConsumerPolicy, PowerPriority};

#[test]
fn legacy_power_consumer_receives_normal_policy_without_changing_demand() {
    let mut world = World::new();
    let consumer = world.spawn(PowerConsumer { demand: 1.25 }).id();
    world.entity_mut(consumer).remove::<PowerConsumerPolicy>();
    assert!(world.get::<PowerConsumerPolicy>(consumer).is_none());

    rehydrate_power_consumer_policies(&mut world);

    assert_eq!(world.get::<PowerConsumer>(consumer).unwrap().demand, 1.25);
    assert_eq!(
        world.get::<PowerConsumerPolicy>(consumer),
        Some(&PowerConsumerPolicy {
            priority: PowerPriority::Normal,
        })
    );
}

#[test]
fn explicit_power_consumer_priority_is_preserved() {
    let mut world = World::new();
    let consumer = world
        .spawn((
            PowerConsumer { demand: 0.5 },
            PowerConsumerPolicy {
                priority: PowerPriority::High,
            },
        ))
        .id();

    rehydrate_power_consumer_policies(&mut world);

    assert_eq!(
        world.get::<PowerConsumerPolicy>(consumer).unwrap().priority,
        PowerPriority::High
    );
}
