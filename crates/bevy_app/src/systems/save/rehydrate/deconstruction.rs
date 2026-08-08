//! Deconstruction-order validation and runtime gate reconstruction.

use bevy::prelude::*;
use hw_core::jobs::WorkType;
use hw_energy::{SoulSpaSite, SoulSpaTile};
use hw_jobs::construction::{
    FloorConstructionSite, FloorTileBlueprint, WallConstructionSite, WallTileBlueprint,
};
use hw_jobs::{
    Blueprint, Building, BuildingType, DeconstructionCommitClaim, DeconstructionOrder,
    DeconstructionOrders, DeconstructionPending, DeconstructionTargetClass, Designation,
    PlayerIssuedDesignation, Priority, TargetDeconstructionRoot, TaskSlots,
    resolve_deconstruction_target,
};
use hw_world::WorldMap;

pub(in crate::systems::save) fn validate_deconstruction_orders(
    candidate: &World,
) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;

    for entity_ref in candidate.iter_entities() {
        let entity = entity_ref.id();
        let order = entity_ref.contains::<DeconstructionOrder>();
        let source = entity_ref.get::<TargetDeconstructionRoot>();
        let deconstruct_designation = entity_ref
            .get::<Designation>()
            .is_some_and(|designation| designation.work_type == WorkType::Deconstruct);

        if source.is_some() && !order {
            return Err(format!(
                "entity {entity:?} has TargetDeconstructionRoot without DeconstructionOrder"
            ));
        }
        if deconstruct_designation && !order {
            return Err(format!(
                "entity {entity:?} has Designation::Deconstruct without the dedicated DeconstructionOrder role"
            ));
        }
        if order {
            validate_order(candidate, map, entity, source)?;
        }

        if let Some(orders) = entity_ref.get::<DeconstructionOrders>() {
            validate_target_collection(candidate, entity, orders)?;
        }
    }

    Ok(())
}

fn validate_order(
    candidate: &World,
    map: &WorldMap,
    order: Entity,
    source: Option<&TargetDeconstructionRoot>,
) -> Result<(), String> {
    let order_ref = candidate
        .get_entity(order)
        .map_err(|_| format!("DeconstructionOrder {order:?} disappeared during validation"))?;
    if order_ref
        .get::<Designation>()
        .is_none_or(|designation| designation.work_type != WorkType::Deconstruct)
    {
        return Err(format!(
            "DeconstructionOrder {order:?} must have Designation::Deconstruct"
        ));
    }
    if !order_ref.contains::<PlayerIssuedDesignation>()
        || !order_ref.contains::<Priority>()
        || !order_ref.contains::<Transform>()
    {
        return Err(format!(
            "DeconstructionOrder {order:?} is missing player provenance, priority, or Transform"
        ));
    }
    if order_ref
        .get::<TaskSlots>()
        .is_none_or(|slots| slots.max != 1)
    {
        return Err(format!(
            "DeconstructionOrder {order:?} must have exactly one task slot"
        ));
    }

    let target = source
        .ok_or_else(|| format!("DeconstructionOrder {order:?} has no target relationship"))?
        .0;
    if target == order {
        return Err(format!(
            "DeconstructionOrder {order:?} cannot target itself"
        ));
    }
    if has_deconstruction_target_role(&order_ref) {
        return Err(format!(
            "DeconstructionOrder {order:?} cannot also have a deconstruction target role"
        ));
    }
    if candidate.get::<DeconstructionOrder>(target).is_some() {
        return Err(format!(
            "DeconstructionOrder {order:?} target {target:?} cannot also have the DeconstructionOrder role"
        ));
    }
    let resolved = resolve_deconstruction_target(candidate, target).map_err(|reason| {
        format!("DeconstructionOrder {order:?} has invalid target {target:?}: {reason:?}")
    })?;
    if resolved.root != target {
        return Err(format!(
            "DeconstructionOrder {order:?} targets non-canonical entity {target:?}; root is {:?}",
            resolved.root
        ));
    }
    let order_position = order_ref
        .get::<Transform>()
        .expect("order Transform presence was checked above")
        .translation
        .truncate();
    let target_position = candidate
        .get::<Transform>(target)
        .ok_or_else(|| format!("DeconstructionOrder {order:?} target {target:?} has no Transform"))?
        .translation
        .truncate();
    if order_position != target_position {
        return Err(format!(
            "DeconstructionOrder {order:?} anchor {order_position:?} does not match target {target:?} position {target_position:?}"
        ));
    }
    let ownership = map.snapshot_owner(target);
    match resolved.class {
        DeconstructionTargetClass::Building(BuildingType::Floor)
            if ownership.floor_grids.len() != 1 || !ownership.building_grids.is_empty() =>
        {
            return Err(format!(
                "DeconstructionOrder {order:?} Floor target {target:?} must own exactly one floor grid and no building grid"
            ));
        }
        DeconstructionTargetClass::Building(BuildingType::Floor) => {}
        DeconstructionTargetClass::Building(_) | DeconstructionTargetClass::SoulSpa
            if ownership.building_grids.is_empty() || !ownership.floor_grids.is_empty() =>
        {
            return Err(format!(
                "DeconstructionOrder {order:?} target {target:?} must own building grids and no floor grid"
            ));
        }
        DeconstructionTargetClass::Building(_) | DeconstructionTargetClass::SoulSpa => {}
    }

    let target_orders = candidate
        .get::<DeconstructionOrders>(target)
        .ok_or_else(|| {
            format!("DeconstructionOrder {order:?} target {target:?} has no reverse relationship")
        })?;
    if target_orders.len() != 1 || !target_orders.iter().any(|candidate| *candidate == order) {
        return Err(format!(
            "DeconstructionOrder {order:?} target {target:?} does not reference it exactly once"
        ));
    }
    Ok(())
}

fn has_deconstruction_target_role(entity: &EntityRef<'_>) -> bool {
    entity.contains::<Building>()
        || entity.contains::<SoulSpaSite>()
        || entity.contains::<SoulSpaTile>()
        || entity.contains::<Blueprint>()
        || entity.contains::<FloorConstructionSite>()
        || entity.contains::<FloorTileBlueprint>()
        || entity.contains::<WallConstructionSite>()
        || entity.contains::<WallTileBlueprint>()
        || entity.contains::<DeconstructionOrders>()
}

/// Rebuilds the stackable completed-Floor lookup from durable entity state.
///
/// `WorldMap.floors` is additive to save v1 and is therefore allowed to be
/// absent in older bodies. Candidate validation has already guaranteed that
/// every completed Floor has one unique canonical grid before this infallible
/// normalization step runs.
pub(in crate::systems::save) fn normalize_completed_floor_ownership(world: &mut World) {
    let mut floors: Vec<((i32, i32), Entity)> = {
        let mut query = world.query::<(Entity, &Building, &Transform)>();
        query
            .iter(world)
            .filter(|(_, building, _)| {
                building.kind == BuildingType::Floor && !building.is_provisional
            })
            .map(|(entity, _, transform)| {
                (
                    WorldMap::world_to_grid(transform.translation.truncate()),
                    entity,
                )
            })
            .collect()
    };
    floors.sort_unstable_by_key(|(grid, entity)| (grid.1, grid.0, entity.to_bits()));
    let mut map = world.resource_mut::<WorldMap>();
    for &(grid, floor) in &floors {
        if map.building_entity(grid) == Some(floor) {
            map.clear_building(grid);
        }
    }
    map.replace_floor_owners(floors);
}

fn validate_target_collection(
    candidate: &World,
    target: Entity,
    orders: &DeconstructionOrders,
) -> Result<(), String> {
    if orders.len() > 1 {
        return Err(format!(
            "deconstruction target {target:?} has {} orders; maximum is one",
            orders.len()
        ));
    }
    for &order in orders.iter() {
        let order_ref = candidate.get_entity(order).map_err(|_| {
            format!("deconstruction target {target:?} references missing order {order:?}")
        })?;
        if !order_ref.contains::<DeconstructionOrder>()
            || order_ref
                .get::<TargetDeconstructionRoot>()
                .is_none_or(|source| source.0 != target)
        {
            return Err(format!(
                "deconstruction target {target:?} has asymmetric order relationship {order:?}"
            ));
        }
    }
    Ok(())
}

/// Rebuilds runtime gates from the validated durable order relationships.
pub(in crate::systems::save) fn rebuild_deconstruction_runtime(world: &mut World) {
    let runtime_entities: Vec<Entity> = {
        let mut query = world.query_filtered::<
            Entity,
            Or<(With<DeconstructionPending>, With<DeconstructionCommitClaim>)>,
        >();
        query.iter(world).collect()
    };
    for entity in runtime_entities {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.remove::<(DeconstructionPending, DeconstructionCommitClaim)>();
        }
    }

    let pending: Vec<(Entity, Entity)> = {
        let mut query = world
            .query_filtered::<(Entity, &TargetDeconstructionRoot), With<DeconstructionOrder>>();
        query
            .iter(world)
            .map(|(order, target)| (target.0, order))
            .collect()
    };
    for (target, order) in pending {
        if let Ok(mut target_mut) = world.get_entity_mut(target) {
            target_mut.insert(DeconstructionPending { order });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::relationship::RelationshipHookMode;

    fn spawn_target(world: &mut World) -> Entity {
        let target = world
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Transform::default(),
            ))
            .id();
        world
            .resource_mut::<WorldMap>()
            .set_building((3, 4), target);
        target
    }

    fn spawn_order(world: &mut World, target: Entity) -> Entity {
        let target_transform = world.get::<Transform>(target).cloned().unwrap_or_default();
        world
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                PlayerIssuedDesignation,
                Priority::default(),
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
                target_transform,
            ))
            .id()
    }

    #[test]
    fn valid_order_rebuilds_pending_idempotently_and_clears_claim() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        let order = spawn_order(&mut world, target);
        world.flush();
        world.entity_mut(target).insert(DeconstructionCommitClaim {
            world_epoch: 7,
            order,
        });

        validate_deconstruction_orders(&world).unwrap();
        rebuild_deconstruction_runtime(&mut world);
        rebuild_deconstruction_runtime(&mut world);

        assert_eq!(
            world.get::<DeconstructionPending>(target),
            Some(&DeconstructionPending { order })
        );
        assert!(world.get::<DeconstructionCommitClaim>(target).is_none());
    }

    #[test]
    fn duplicate_orders_for_one_target_fail_closed() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        spawn_order(&mut world, target);
        spawn_order(&mut world, target);
        world.flush();

        let error = validate_deconstruction_orders(&world).unwrap_err();
        assert!(
            error.contains("maximum is one") || error.contains("exactly once"),
            "{error}"
        );
    }

    #[test]
    fn malformed_order_shape_fails_closed() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        world.spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Chop,
            },
            TargetDeconstructionRoot(target),
            Transform::default(),
        ));
        world.flush();

        let error = validate_deconstruction_orders(&world).unwrap_err();
        assert!(error.contains("Designation::Deconstruct"), "{error}");
    }

    #[test]
    fn order_anchor_must_match_the_canonical_target() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        let order = spawn_order(&mut world, target);
        world.flush();
        world
            .entity_mut(order)
            .insert(Transform::from_xyz(32.0, 0.0, 0.0));

        let error = validate_deconstruction_orders(&world).unwrap_err();
        assert!(error.contains("does not match target"), "{error}");
    }

    #[test]
    fn legacy_completed_floor_ownership_is_rebuilt_into_the_stackable_layer() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let grid = (6, 7);
        let floor = world
            .spawn((
                Building {
                    kind: BuildingType::Floor,
                    is_provisional: false,
                },
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            ))
            .id();
        world.resource_mut::<WorldMap>().set_building(grid, floor);

        normalize_completed_floor_ownership(&mut world);

        assert_eq!(world.resource::<WorldMap>().floor_entity(grid), Some(floor));
        assert_eq!(world.resource::<WorldMap>().building_entity(grid), None);
    }

    #[test]
    fn floor_order_uses_the_floor_owner_layer() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let grid = (8, 9);
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        let floor = world
            .spawn((
                Building {
                    kind: BuildingType::Floor,
                    is_provisional: false,
                },
                Transform::from_translation(position.extend(0.0)),
            ))
            .id();
        world.resource_mut::<WorldMap>().set_floor(grid, floor);
        spawn_order(&mut world, floor);
        world.flush();

        validate_deconstruction_orders(&world).unwrap();
    }

    #[test]
    fn hook_skipped_self_target_order_fails_closed() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let order = world
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                PlayerIssuedDesignation,
                Priority::default(),
                TaskSlots::new(1),
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Transform::default(),
            ))
            .id();
        world.entity_mut(order).insert_with_relationship_hook_mode(
            TargetDeconstructionRoot(order),
            RelationshipHookMode::Skip,
        );
        world.resource_mut::<WorldMap>().set_building((3, 4), order);

        let error = validate_deconstruction_orders(&world).unwrap_err();

        assert!(error.contains("cannot target itself"), "{error}");
    }

    #[test]
    fn target_cannot_also_have_the_order_role() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        world.entity_mut(target).insert(DeconstructionOrder);
        let order = spawn_order(&mut world, target);
        world.flush();
        let source = world
            .get::<TargetDeconstructionRoot>(order)
            .expect("spawned order target");

        let error =
            validate_order(&world, world.resource::<WorldMap>(), order, Some(source)).unwrap_err();

        assert!(error.contains("cannot also have the DeconstructionOrder role"));
    }

    #[test]
    fn deconstruct_designation_cannot_be_attached_directly_to_a_target() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        world.entity_mut(target).insert(Designation {
            work_type: WorkType::Deconstruct,
        });

        let error = validate_deconstruction_orders(&world).unwrap_err();

        assert!(error.contains("without the dedicated DeconstructionOrder role"));
    }

    #[test]
    fn order_cannot_also_be_a_completed_target_root() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        let target = spawn_target(&mut world);
        let order = spawn_order(&mut world, target);
        world.entity_mut(order).insert(Building {
            kind: BuildingType::Wall,
            is_provisional: false,
        });
        world.flush();

        let error = validate_deconstruction_orders(&world).unwrap_err();

        assert!(error.contains("cannot also have a deconstruction target role"));
    }
}
