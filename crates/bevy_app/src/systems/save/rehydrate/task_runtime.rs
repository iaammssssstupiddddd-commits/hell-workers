//! Runtime-only task and logistics state reconstructed after a world replace.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::Z_ITEM_PICKUP;
use hw_core::logistics::ResourceType;
use hw_core::relationships::{DeliveringTo, LoadedIn, ParkedAt, PushedBy, WorkingOn};
use hw_core::soul::DamnedSoul;
use hw_jobs::construction::{TargetFloorConstructionSite, TargetWallConstructionSite};
use hw_jobs::mud_mixer::TargetMixer;
use hw_jobs::{AssignedTask, TargetBlueprint, TargetSoulSpaSite};
use hw_logistics::item_lifetime::ItemDespawnTimer;
use hw_logistics::transport_request::{
    TransportDemand, TransportRequest, TransportRequestState, WheelbarrowLease,
    WheelbarrowPendingSince,
};
use hw_logistics::{BelongsTo, Inventory, ResourceItem, Wheelbarrow};

use super::obstacles::DurableNavigationView;

enum InventoryDrop {
    Item {
        soul: Entity,
        entity: Entity,
        position: Vec2,
    },
    Wheelbarrow {
        entity: Entity,
        parking: Entity,
        position: Vec2,
    },
}

/// Restores producer lookup markers omitted by pre-C3 floor/wall saves and
/// canonicalizes every deterministic request target from kind+anchor.
pub(super) fn normalize_transport_request_targets(world: &mut World) {
    let requests: Vec<_> = {
        let mut query = world.query::<(Entity, &TransportRequest)>();
        query
            .iter(world)
            .map(|(entity, request)| (entity, request.kind, request.anchor))
            .collect()
    };
    for (entity, kind, anchor) in requests {
        let mut request = world.entity_mut(entity);
        request.remove::<(
            TargetBlueprint,
            TargetFloorConstructionSite,
            TargetWallConstructionSite,
            TargetMixer,
            TargetSoulSpaSite,
        )>();
        match kind {
            hw_logistics::transport_request::TransportRequestKind::DeliverToBlueprint => {
                request.insert(TargetBlueprint(anchor));
            }
            hw_logistics::transport_request::TransportRequestKind::DeliverToFloorConstruction => {
                request.insert(TargetFloorConstructionSite(anchor));
            }
            hw_logistics::transport_request::TransportRequestKind::DeliverToWallConstruction => {
                request.insert(TargetWallConstructionSite(anchor));
            }
            hw_logistics::transport_request::TransportRequestKind::DeliverToMixerSolid
            | hw_logistics::transport_request::TransportRequestKind::DeliverWaterToMixer => {
                request.insert(TargetMixer(anchor));
            }
            hw_logistics::transport_request::TransportRequestKind::DeliverToSoulSpa => {
                request.insert(TargetSoulSpaSite(anchor));
            }
            _ => {}
        }
    }
}

pub(super) fn normalize_task_logistics_runtime(world: &mut World) {
    remove_runtime_relationship_sources::<WorkingOn>(world);
    remove_runtime_relationship_sources::<DeliveringTo>(world);
    remove_runtime_relationship_sources::<PushedBy>(world);
    restore_default_assigned_tasks(world);
    reset_transport_request_claims(world);

    let inventory_drops = collect_inventory_drops(world);
    // An in-progress wheelbarrow destination lives in AssignedTask/lease state,
    // which is intentionally discarded. Keep LoadedIn through staging so its
    // carrier location survives entity remapping, then return the cargo to the
    // carrier (or carrying Soul) before making the wheelbarrow reusable.
    let loaded_cargo_drops = collect_loaded_cargo_drops(world, &inventory_drops);
    unload_loaded_cargo(world, &loaded_cargo_drops);
    let held_wheelbarrows: HashSet<_> = inventory_drops
        .iter()
        .filter_map(|drop| match drop {
            InventoryDrop::Wheelbarrow { entity, .. } => Some(*entity),
            InventoryDrop::Item { .. } => None,
        })
        .collect();
    let mut normalized_drop_entities: HashSet<_> = inventory_drops
        .iter()
        .map(|drop| match drop {
            InventoryDrop::Item { entity, .. } | InventoryDrop::Wheelbarrow { entity, .. } => {
                *entity
            }
        })
        .collect();
    normalized_drop_entities.extend(loaded_cargo_drops.iter().map(|(entity, _)| *entity));
    normalize_ground_resource_items(world, &normalized_drop_entities);
    normalize_parked_wheelbarrows(world, &held_wheelbarrows);
    let other_wheelbarrows = collect_unparked_wheelbarrows(world, &held_wheelbarrows);

    {
        let mut commands = world.commands();
        for drop in inventory_drops {
            match drop {
                InventoryDrop::Item {
                    soul,
                    entity,
                    position,
                } => {
                    hw_soul_ai::soul_ai::execute::task_execution::common::drop_item(
                        &mut commands,
                        soul,
                        entity,
                        position,
                    );
                }
                InventoryDrop::Wheelbarrow {
                    entity,
                    parking,
                    position,
                } => {
                    hw_soul_ai::soul_ai::execute::task_execution::transport_common::wheelbarrow::park_wheelbarrow_entity(
                        &mut commands,
                        entity,
                        Some(parking),
                        position,
                    );
                }
            }
        }
        for (wheelbarrow, parking, position) in other_wheelbarrows {
            hw_soul_ai::soul_ai::execute::task_execution::transport_common::wheelbarrow::park_wheelbarrow_entity(
                &mut commands,
                wheelbarrow,
                Some(parking),
                position,
            );
        }
    }

    attach_item_lifetimes(world);
}

fn collect_loaded_cargo_drops(
    world: &mut World,
    inventory_drops: &[InventoryDrop],
) -> Vec<(Entity, Vec2)> {
    let carried_wheelbarrow_positions: HashMap<_, _> = inventory_drops
        .iter()
        .filter_map(|drop| match drop {
            InventoryDrop::Wheelbarrow {
                entity, position, ..
            } => Some((*entity, *position)),
            InventoryDrop::Item { .. } => None,
        })
        .collect();
    let loaded: Vec<_> = {
        let mut query = world.query::<(Entity, &LoadedIn)>();
        query
            .iter(world)
            .map(|(item, carrier)| (item, carrier.0))
            .collect()
    };
    let navigation = DurableNavigationView::from_world(world)
        .expect("candidate validation must require durable navigation inputs");
    loaded
        .into_iter()
        .map(|(item, carrier)| {
            let carrier_position = carried_wheelbarrow_positions
                .get(&carrier)
                .copied()
                .or_else(|| {
                    world
                        .get::<Transform>(carrier)
                        .map(|transform| transform.translation.truncate())
                })
                .expect("candidate validation must require a carrier drop position");
            let drop_position = navigation
                .nearest_walkable_position(carrier_position)
                .expect("candidate validation must require a walkable carrier drop cell");
            (item, drop_position)
        })
        .collect()
}

fn unload_loaded_cargo(world: &mut World, cargo: &[(Entity, Vec2)]) {
    for &(item, position) in cargo {
        let mut item = world.entity_mut(item);
        item.remove::<LoadedIn>();
        item.insert((
            Visibility::Visible,
            Transform::from_xyz(position.x, position.y, Z_ITEM_PICKUP),
        ));
    }
}

fn remove_runtime_relationship_sources<T: Component>(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).collect()
    };
    for entity in entities {
        world.entity_mut(entity).remove::<T>();
    }
}

fn restore_default_assigned_tasks(world: &mut World) {
    let souls: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, (With<DamnedSoul>, Without<AssignedTask>)>();
        query.iter(world).collect()
    };
    for soul in souls {
        world.entity_mut(soul).insert(AssignedTask::default());
    }
}

fn reset_transport_request_claims(world: &mut World) {
    let requests: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<TransportRequest>>();
        query.iter(world).collect()
    };
    for request in requests {
        let mut entity = world.entity_mut(request);
        entity.insert(TransportRequestState::Pending);
        entity.remove::<(WheelbarrowLease, WheelbarrowPendingSince)>();
        if let Some(mut demand) = entity.get_mut::<TransportDemand>() {
            demand.inflight = 0;
        }
    }
}

fn collect_inventory_drops(world: &mut World) -> Vec<InventoryDrop> {
    let carried: Vec<(Entity, Entity, Vec2)> = {
        let mut query =
            world.query_filtered::<(Entity, &Inventory, &Transform), With<DamnedSoul>>();
        query
            .iter(world)
            .filter_map(|(soul, inventory, transform)| {
                inventory
                    .0
                    .map(|held| (soul, held, transform.translation.truncate()))
            })
            .collect()
    };
    let navigation = DurableNavigationView::from_world(world)
        .expect("candidate validation must require durable navigation inputs");
    let mut drops = Vec::with_capacity(carried.len());
    for &(soul, held, soul_position) in &carried {
        let drop_position = navigation
            .nearest_walkable_position(soul_position)
            .expect("candidate validation must require a walkable inventory drop cell");
        let Some(held_ref) = world.get_entity(held).ok() else {
            debug_assert!(
                false,
                "candidate validation must reject missing inventory entity"
            );
            continue;
        };
        let drop = if held_ref.contains::<Wheelbarrow>() {
            let Some(parking) = held_ref.get::<BelongsTo>().map(|owner| owner.0) else {
                debug_assert!(
                    false,
                    "candidate validation must reject a wheelbarrow without a home"
                );
                continue;
            };
            InventoryDrop::Wheelbarrow {
                entity: held,
                parking,
                position: drop_position,
            }
        } else if held_ref.contains::<ResourceItem>() {
            InventoryDrop::Item {
                soul,
                entity: held,
                position: drop_position,
            }
        } else {
            debug_assert!(
                false,
                "candidate validation must reject unsupported inventory contents"
            );
            continue;
        };
        drops.push(drop);
    }
    drop(navigation);
    for (soul, _, _) in carried {
        if let Some(mut inventory) = world.get_mut::<Inventory>(soul) {
            inventory.0 = None;
        }
    }
    drops
}

fn collect_unparked_wheelbarrows(
    world: &mut World,
    held_wheelbarrows: &HashSet<Entity>,
) -> Vec<(Entity, Entity, Vec2)> {
    let wheelbarrows: Vec<_> = {
        let mut query = world.query_filtered::<
            (Entity, &BelongsTo, &Transform),
            (With<Wheelbarrow>, Without<ParkedAt>),
        >();
        query
            .iter(world)
            .map(|(entity, owner, transform)| (entity, owner.0, transform.translation.truncate()))
            .collect()
    };
    let navigation = DurableNavigationView::from_world(world)
        .expect("candidate validation must require durable navigation inputs");
    wheelbarrows
        .into_iter()
        .filter(|(entity, _, _)| !held_wheelbarrows.contains(entity))
        .map(|(entity, owner, position)| {
            let position = navigation
                .nearest_walkable_position(position)
                .expect("candidate validation must require a walkable wheelbarrow cell");
            (entity, owner, position)
        })
        .collect()
}

fn normalize_parked_wheelbarrows(world: &mut World, held_wheelbarrows: &HashSet<Entity>) {
    let parked: Vec<(Entity, Vec2)> = {
        let mut query =
            world.query_filtered::<(Entity, &Transform), (With<Wheelbarrow>, With<ParkedAt>)>();
        query
            .iter(world)
            .filter_map(|(entity, transform)| {
                (!held_wheelbarrows.contains(&entity))
                    .then_some((entity, transform.translation.truncate()))
            })
            .collect()
    };
    let navigation = DurableNavigationView::from_world(world)
        .expect("candidate validation must require durable navigation inputs");
    let normalized: Vec<_> = parked
        .into_iter()
        .map(|(entity, position)| {
            let position = navigation
                .nearest_walkable_position(position)
                .expect("candidate validation must require a walkable wheelbarrow cell");
            (entity, position)
        })
        .collect();
    drop(navigation);
    for (entity, position) in normalized {
        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            transform.translation.x = position.x;
            transform.translation.y = position.y;
        }
    }
}

fn normalize_ground_resource_items(world: &mut World, inventory_drops: &HashSet<Entity>) {
    let ground_items: Vec<(Entity, Vec2)> = {
        let mut query = world.query_filtered::<(Entity, &Transform), (
            With<ResourceItem>,
            Without<Wheelbarrow>,
            Without<hw_core::relationships::StoredIn>,
            Without<hw_jobs::mud_mixer::StoredByMixer>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, transform)| {
                (!inventory_drops.contains(&entity))
                    .then_some((entity, transform.translation.truncate()))
            })
            .collect()
    };
    let navigation = DurableNavigationView::from_world(world)
        .expect("candidate validation must require durable navigation inputs");
    let normalized: Vec<_> = ground_items
        .into_iter()
        .map(|(entity, position)| {
            let position = navigation
                .nearest_walkable_position(position)
                .expect("candidate validation must require a walkable ground item cell");
            (entity, position)
        })
        .collect();
    drop(navigation);

    for (entity, position) in normalized {
        let mut item = world.entity_mut(entity);
        item.insert(Visibility::Visible);
        if let Some(mut transform) = item.get_mut::<Transform>() {
            transform.translation = position.extend(Z_ITEM_PICKUP);
        }
    }
}

fn attach_item_lifetimes(world: &mut World) {
    let expiring_items: Vec<Entity> = {
        let mut query = world.query::<(Entity, &ResourceItem)>();
        query
            .iter(world)
            .filter_map(|(entity, item)| {
                matches!(item.0, ResourceType::Sand | ResourceType::StasisMud).then_some(entity)
            })
            .collect()
    };
    for item in expiring_items {
        world.entity_mut(item).insert(ItemDespawnTimer::new(5.0));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use hw_core::constants::Z_ITEM_PICKUP;
    use hw_core::relationships::{IncomingDeliveries, LoadedIn, TaskWorkers};
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequestKind, WheelbarrowDestination,
    };
    use hw_logistics::types::WheelbarrowParking;
    use hw_world::WorldMap;

    fn request(anchor: Entity) -> TransportRequest {
        TransportRequest {
            kind: TransportRequestKind::DepositToStockpile,
            anchor,
            resource_type: ResourceType::Wood,
            issued_by: anchor,
            priority: TransportPriority::Normal,
            stockpile_group: Vec::new(),
        }
    }

    #[test]
    fn normalization_clears_claims_and_reopens_transport_demand() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let anchor = world.spawn_empty().id();
        let request_entity = world
            .spawn((
                request(anchor),
                TransportRequestState::Claimed,
                TransportDemand {
                    desired_slots: 2,
                    inflight: 2,
                },
                WheelbarrowPendingSince(3.0),
                WheelbarrowLease {
                    wheelbarrow: anchor,
                    items: Vec::new(),
                    source_pos: Vec2::ZERO,
                    destination: WheelbarrowDestination::Stockpile(anchor),
                    lease_until: 9.0,
                },
            ))
            .id();
        let soul = world
            .spawn((
                DamnedSoul::default(),
                AssignedTask::None,
                WorkingOn(request_entity),
            ))
            .id();
        let item = world
            .spawn((
                ResourceItem(ResourceType::Wood),
                DeliveringTo(request_entity),
            ))
            .id();
        world.flush();

        normalize_task_logistics_runtime(&mut world);
        world.flush();

        assert!(world.get::<WorkingOn>(soul).is_none());
        assert!(world.get::<TaskWorkers>(request_entity).is_none());
        assert!(world.get::<DeliveringTo>(item).is_none());
        assert!(world.get::<IncomingDeliveries>(request_entity).is_none());
        assert_eq!(
            world.get::<TransportRequestState>(request_entity),
            Some(&TransportRequestState::Pending)
        );
        assert_eq!(
            world
                .get::<TransportDemand>(request_entity)
                .unwrap()
                .inflight,
            0
        );
        assert_eq!(
            world
                .get::<TransportDemand>(request_entity)
                .unwrap()
                .desired_slots,
            2
        );
        assert!(world.get::<WheelbarrowLease>(request_entity).is_none());
        assert!(
            world
                .get::<WheelbarrowPendingSince>(request_entity)
                .is_none()
        );
    }

    #[test]
    fn normalization_drops_inventory_parks_tools_and_restores_item_lifetimes() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
        let wheelbarrow = world
            .spawn((
                Wheelbarrow { capacity: 2 },
                BelongsTo(parking),
                Transform::from_xyz(480.0, 480.0, 0.0),
            ))
            .id();
        let soul = world
            .spawn((
                DamnedSoul::default(),
                Inventory(Some(wheelbarrow)),
                Transform::from_xyz(64.0, 64.0, 0.0),
            ))
            .id();
        world.entity_mut(wheelbarrow).insert(PushedBy(soul));
        let sand = world
            .spawn((
                ResourceItem(ResourceType::Sand),
                LoadedIn(wheelbarrow),
                Transform::from_xyz(-640.0, -640.0, 0.0),
            ))
            .id();
        let mud = world
            .spawn((ResourceItem(ResourceType::StasisMud), Transform::default()))
            .id();
        let wood = world
            .spawn((ResourceItem(ResourceType::Wood), Transform::default()))
            .id();
        world.flush();
        let expected_soul_drop = DurableNavigationView::from_world(&world)
            .unwrap()
            .nearest_walkable_position(Vec2::splat(64.0))
            .unwrap();

        normalize_task_logistics_runtime(&mut world);
        world.flush();

        assert_eq!(world.get::<Inventory>(soul).unwrap().0, None);
        assert_eq!(world.get::<ParkedAt>(wheelbarrow).unwrap().0, parking);
        assert!(world.get::<PushedBy>(wheelbarrow).is_none());
        assert!(world.get::<LoadedIn>(sand).is_none());
        assert_eq!(world.get::<Visibility>(sand), Some(&Visibility::Visible));
        assert_eq!(
            world.get::<Transform>(sand).unwrap().translation.z,
            Z_ITEM_PICKUP
        );
        assert_eq!(
            world.get::<Transform>(sand).unwrap().translation.truncate(),
            expected_soul_drop
        );
        for item in [sand, mud] {
            let timer = &world.get::<ItemDespawnTimer>(item).unwrap().0;
            assert_eq!(timer.duration(), Duration::from_secs(5));
            assert_eq!(timer.elapsed(), Duration::ZERO);
            assert_eq!(timer.mode(), TimerMode::Once);
        }
        assert!(world.get::<ItemDespawnTimer>(wood).is_none());
    }

    #[test]
    fn target_normalization_replaces_stale_markers_from_kind_and_anchor() {
        let mut world = World::new();
        let anchor = world
            .spawn(hw_jobs::Blueprint::new(
                hw_jobs::BuildingType::Tank,
                vec![(2, 3)],
            ))
            .id();
        let stale = world.spawn_empty().id();
        let request = world
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToBlueprint,
                    anchor,
                    resource_type: ResourceType::Wood,
                    issued_by: stale,
                    priority: TransportPriority::Normal,
                    stockpile_group: Vec::new(),
                },
                TargetMixer(stale),
                TargetSoulSpaSite(stale),
            ))
            .id();

        normalize_transport_request_targets(&mut world);

        assert_eq!(world.get::<TargetBlueprint>(request).unwrap().0, anchor);
        assert!(world.get::<TargetMixer>(request).is_none());
        assert!(world.get::<TargetSoulSpaSite>(request).is_none());
        assert!(world.get::<TargetFloorConstructionSite>(request).is_none());
        assert!(world.get::<TargetWallConstructionSite>(request).is_none());
    }

    #[test]
    fn normalization_unloads_cargo_at_the_parked_carrier_not_the_old_pickup_transform() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
        let wheelbarrow = world
            .spawn((
                Wheelbarrow { capacity: 2 },
                BelongsTo(parking),
                ParkedAt(parking),
                Transform::from_xyz(96.0, 64.0, 0.0),
            ))
            .id();
        let wood = world
            .spawn((
                ResourceItem(ResourceType::Wood),
                LoadedIn(wheelbarrow),
                Transform::from_xyz(640.0, 640.0, 0.0),
            ))
            .id();
        world.flush();
        let expected_carrier_drop = DurableNavigationView::from_world(&world)
            .unwrap()
            .nearest_walkable_position(Vec2::new(96.0, 64.0))
            .unwrap();

        normalize_task_logistics_runtime(&mut world);
        world.flush();

        assert!(world.get::<LoadedIn>(wood).is_none());
        assert_eq!(world.get::<Visibility>(wood), Some(&Visibility::Visible));
        assert_eq!(
            world.get::<Transform>(wood).unwrap().translation.truncate(),
            expected_carrier_drop
        );
    }

    #[test]
    fn normalization_drops_carried_expiring_item_to_a_walkable_cell() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let sand = world
            .spawn((
                ResourceItem(ResourceType::Sand),
                Transform::from_xyz(999.0, 999.0, 0.0),
            ))
            .id();
        let soul = world
            .spawn((
                DamnedSoul::default(),
                Inventory(Some(sand)),
                Transform::from_xyz(32.0, 32.0, 0.0),
            ))
            .id();

        normalize_task_logistics_runtime(&mut world);
        world.flush();

        assert_eq!(world.get::<Inventory>(soul).unwrap().0, None);
        assert_eq!(
            world.get::<Transform>(sand).unwrap().translation.z,
            Z_ITEM_PICKUP
        );
        assert!(world.get::<ItemDespawnTimer>(sand).is_some());
    }
}
