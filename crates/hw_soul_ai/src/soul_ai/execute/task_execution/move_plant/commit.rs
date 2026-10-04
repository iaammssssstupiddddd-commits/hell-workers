use super::*;
use hw_core::relationships::WorkingOn;
use hw_jobs::{
    ActiveTaskIdentity, DeconstructionPending, MovePlantTask, ObstaclePosition, ObstacleSourceKind,
};
use hw_logistics::{BelongsTo, BucketStorage};
use std::collections::HashSet;

struct PreparedMove {
    obstacles: Vec<(Entity, (i32, i32))>,
    reservations: Vec<Entity>,
    stockpiles: Vec<StockpileMove>,
    transforms: Vec<(Entity, Transform)>,
    retained_blockers: HashSet<(i32, i32)>,
}

struct StockpileMove {
    entity: Entity,
    old: (i32, i32),
    new: (i32, i32),
}

fn matches_worker(world: &World, building: Entity, pending: &PendingBuildingMove) -> bool {
    world.get::<ActiveTaskIdentity>(pending.worker) == Some(&pending.expected_identity)
        && pending.expected_identity.matches_working_on(
            world
                .get::<WorkingOn>(pending.worker)
                .map(|working| working.0),
        )
        && matches!(world.get::<AssignedTask>(pending.worker), Some(AssignedTask::MovePlant(data))
            if data.task_entity == pending.task_entity && data.building == building && data.phase == MovePlantPhase::Moving)
}

fn prepare_move(
    world: &mut World,
    building: Entity,
    pending: &PendingBuildingMove,
) -> Option<PreparedMove> {
    let task = world.get::<MovePlantTask>(pending.task_entity)?;
    let AssignedTask::MovePlant(data) = world.get::<AssignedTask>(pending.worker)? else {
        return None;
    };
    if world
        .get::<MovePlanned>(building)
        .is_none_or(|planned| planned.task_entity != pending.task_entity)
        || task.building != building
        || task.destination_grid != data.destination_grid
        || task.destination_pos != data.destination_pos
        || task.companion_anchor != data.companion_anchor
        || task.companion_anchor != pending.companion_anchor
        || world.get::<Transform>(building) != Some(&pending.expected_transform)
        || world.get::<Building>(building)?.kind != pending.expected_kind
        || world.get::<DeconstructionPending>(building).is_some()
        || !matches!(
            pending.expected_kind,
            BuildingType::Tank | BuildingType::MudMixer
        )
    {
        return None;
    }
    let old_anchor = anchor_grid_for_kind(
        pending.expected_kind,
        pending.expected_transform.translation.truncate(),
    );
    let new_anchor = task.destination_grid;
    let delta = (new_anchor.0 - old_anchor.0, new_anchor.1 - old_anchor.1);
    if occupied_grids_for_kind(pending.expected_kind, old_anchor) != pending.old_occupied
        || occupied_grids_for_kind(pending.expected_kind, new_anchor) != pending.new_occupied
        || pending.proposed_transform.translation.truncate()
            != spawn_pos_for_kind(pending.expected_kind, new_anchor)
    {
        return None;
    }
    let reservation = world.get::<MovePlantReservation>(pending.task_entity)?;
    let reserved: HashSet<_> = reservation.occupied.iter().copied().collect();
    if reserved.len() != reservation.occupied.len() {
        return None;
    }
    let mut obstacles = Vec::new();
    let mut reservations = Vec::new();
    let mut reserved_markers = HashSet::new();
    let mut old_markers = HashSet::new();
    let mut retained_blockers = HashSet::new();
    for (entity, position, kind, parent) in world
        .query::<(
            Entity,
            &ObstaclePosition,
            Option<&ObstacleSourceKind>,
            Option<&ChildOf>,
        )>()
        .iter(world)
    {
        let grid = (position.0, position.1);
        if parent.is_some_and(|parent| parent.parent() == building) {
            if kind != Some(&ObstacleSourceKind::BuildingFootprint)
                || !pending.old_occupied.contains(&grid)
                || !old_markers.insert(grid)
            {
                return None;
            }
            obstacles.push((entity, (grid.0 + delta.0, grid.1 + delta.1)));
        } else if parent.is_some_and(|parent| parent.parent() == pending.task_entity) {
            if kind != Some(&ObstacleSourceKind::PlacementReservation)
                || !reserved.contains(&grid)
                || !reserved_markers.insert(grid)
            {
                return None;
            }
            reservations.push(entity);
        } else {
            retained_blockers.insert(grid);
        }
    }
    if reserved_markers != reserved || old_markers.len() != pending.old_occupied.len() {
        return None;
    }

    let companions: Vec<_> = world
        .query_filtered::<(Entity, &BelongsTo, &Transform), With<BucketStorage>>()
        .iter(world)
        .filter(|(_, owner, _)| owner.0 == building)
        .map(|(entity, _, transform)| (entity, *transform))
        .collect();
    let map = world.resource::<WorldMap>();
    map.validate_owned_footprint(building, pending.old_occupied.iter().copied())
        .ok()?;
    if map
        .snapshot_owner(building)
        .building_grids
        .into_iter()
        .collect::<HashSet<_>>()
        != pending.old_occupied.iter().copied().collect()
    {
        return None;
    }
    let mut old_stockpiles = Vec::new();
    for (entity, transform) in companions {
        let grids: Vec<_> = map
            .stockpile_entries()
            .filter(|(_, owner)| **owner == entity)
            .map(|(grid, _)| *grid)
            .collect();
        if grids.len() != 1 || WorldMap::world_to_grid(transform.translation.truncate()) != grids[0]
        {
            return None;
        }
        old_stockpiles.push((entity, grids[0], transform));
    }
    old_stockpiles.sort_by_key(|(_, grid, _)| *grid);
    if (pending.expected_kind == BuildingType::Tank
        && (old_stockpiles.len() != 2 || pending.companion_anchor.is_none()))
        || (pending.expected_kind != BuildingType::Tank
            && (!old_stockpiles.is_empty() || pending.companion_anchor.is_some()))
    {
        return None;
    }
    let mut stockpiles = Vec::new();
    let mut transforms = vec![(building, pending.proposed_transform)];
    for (index, &(entity, old, mut transform)) in old_stockpiles.iter().enumerate() {
        let anchor = pending.companion_anchor?;
        let new = (anchor.0 + index as i32, anchor.1);
        let position = WorldMap::grid_to_world(new.0, new.1);
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        stockpiles.push(StockpileMove { entity, old, new });
        transforms.push((entity, transform));
    }
    let new_stockpiles: HashSet<_> = stockpiles.iter().map(|moved| moved.new).collect();
    let destination: HashSet<_> = pending
        .new_occupied
        .iter()
        .copied()
        .chain(new_stockpiles.iter().copied())
        .collect();
    if destination != reserved || destination.len() != pending.new_occupied.len() + stockpiles.len()
    {
        return None;
    }
    for &grid in &destination {
        if !map.has_raw_obstacle(grid.0, grid.1)
            || !map.is_walkable_with_raw_obstacle(grid.0, grid.1, false)
            || map
                .building_entity(grid)
                .is_some_and(|owner| owner != building)
            || map
                .stockpile_entity(grid)
                .is_some_and(|owner| !old_stockpiles.iter().any(|(entity, _, _)| *entity == owner))
            || retained_blockers.contains(&grid)
        {
            return None;
        }
    }
    let offset = Vec3::new(delta.0 as f32 * TILE_SIZE, delta.1 as f32 * TILE_SIZE, 0.0);
    for (entity, owner, transform) in world.query_filtered::<(Entity, &BelongsTo, &Transform), (Without<BucketStorage>, Without<Building>)>().iter(world) {
        if owner.0 == building {
            let mut next = *transform;
            next.translation += offset;
            transforms.push((entity, next));
        }
    }
    Some(PreparedMove {
        obstacles,
        reservations,
        stockpiles,
        transforms,
        retained_blockers,
    })
}

fn commit_move(
    world: &mut World,
    building: Entity,
    pending: &PendingBuildingMove,
    prepared: PreparedMove,
) {
    {
        let mut map = world.resource_mut::<WorldMap>();
        for &grid in &pending.old_occupied {
            map.clear_building_if_owned(grid, building);
        }
        for &grid in &pending.new_occupied {
            map.set_building(grid, building);
        }
        // Clear all sources before inserting destinations, including overlapping moves.
        for moved in &prepared.stockpiles {
            map.clear_stockpile_tile_if_owned(moved.old, moved.entity);
        }
        for moved in &prepared.stockpiles {
            map.set_stockpile(moved.new, moved.entity);
        }
        let affected: HashSet<_> = pending
            .old_occupied
            .iter()
            .copied()
            .chain(pending.new_occupied.iter().copied())
            .chain(prepared.stockpiles.iter().map(|moved| moved.new))
            .collect();
        for grid in affected {
            if pending.new_occupied.contains(&grid) || prepared.retained_blockers.contains(&grid) {
                map.add_grid_obstacle(grid);
            } else {
                map.remove_grid_obstacle(grid);
            }
        }
    }
    for (entity, grid) in prepared.obstacles {
        *world.get_mut::<ObstaclePosition>(entity).unwrap() = ObstaclePosition(grid.0, grid.1);
    }
    for entity in prepared.reservations {
        world.entity_mut(entity).despawn();
    }
    for (entity, transform) in prepared.transforms {
        *world.get_mut::<Transform>(entity).unwrap() = transform;
    }
    world
        .entity_mut(pending.task_entity)
        .remove::<MovePlantReservation>();
    world.entity_mut(building).remove::<PendingBuildingMove>();
    if let AssignedTask::MovePlant(data) =
        &mut *world.get_mut::<AssignedTask>(pending.worker).unwrap()
    {
        data.phase = MovePlantPhase::Done;
    }
}

pub fn apply_pending_building_move_system(world: &mut World) {
    let pending: Vec<_> = world
        .query::<(Entity, &PendingBuildingMove)>()
        .iter(world)
        .map(|(entity, pending)| (entity, pending.clone()))
        .collect();
    for (building, pending) in pending {
        if !matches_worker(world, building, &pending) {
            world.entity_mut(building).remove::<PendingBuildingMove>();
        } else if pending.rejected {
            continue;
        } else if let Some(prepared) = prepare_move(world, building, &pending) {
            commit_move(world, building, &pending, prepared);
        } else {
            world
                .get_mut::<PendingBuildingMove>(building)
                .unwrap()
                .rejected = true;
        }
    }
}
