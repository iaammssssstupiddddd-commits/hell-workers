use super::{super::*, validate_world_map_grid};

pub(in super::super) fn validate_world_map_candidate(candidate: &World) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;
    let expected_len = (MAP_WIDTH * MAP_HEIGHT) as usize;
    for (label, actual_len) in [
        ("tiles", map.tiles.len()),
        ("tile_entities", map.tile_entities.len()),
        ("obstacles", map.obstacles.len()),
    ] {
        if actual_len != expected_len {
            return Err(format!(
                "WorldMap.{label} has length {actual_len}, expected {expected_len}"
            ));
        }
    }

    let mut tile_entities = HashSet::new();
    for (index, tile) in map.tile_entities.iter().enumerate() {
        let Some(tile) = tile else {
            continue;
        };
        if !tile_entities.insert(*tile) {
            return Err(format!(
                "WorldMap.tile_entities references Tile {tile:?} more than once"
            ));
        }
        let tile_ref = candidate.get_entity(*tile).map_err(|_| {
            format!("WorldMap.tile_entities[{index}] references missing Tile {tile:?}")
        })?;
        if !tile_ref.contains::<Tile>() {
            return Err(format!(
                "WorldMap.tile_entities[{index}] target {tile:?} is not a Tile"
            ));
        }
        if !tile_ref.contains::<Transform>() {
            return Err(format!(
                "WorldMap.tile_entities[{index}] Tile {tile:?} has no Transform"
            ));
        }
    }
    let persisted_tiles: HashSet<_> = candidate
        .iter_entities()
        .filter(|entity| entity.contains::<Tile>())
        .map(|entity| entity.id())
        .collect();
    match tile_entities.len() {
        0 if persisted_tiles.is_empty() => {}
        0 => {
            return Err(format!(
                "sparse WorldMap has {} orphan Tile entities",
                persisted_tiles.len()
            ));
        }
        count if count == expected_len && persisted_tiles == tile_entities => {}
        count if count == expected_len => {
            return Err(
                "legacy WorldMap Tile entities do not exactly match tile_entities".to_owned(),
            );
        }
        count => {
            return Err(format!(
                "WorldMap.tile_entities has mixed sparse/legacy shape ({count}/{expected_len} anchors)"
            ));
        }
    }

    for (&grid, &owner) in &map.buildings {
        validate_world_map_grid("buildings", grid)?;
        let owner_ref = candidate.get_entity(owner).map_err(|_| {
            format!("WorldMap.buildings[{grid:?}] references missing entity {owner:?}")
        })?;
        if !owner_ref.contains::<Building>()
            && !owner_ref.contains::<Blueprint>()
            && !owner_ref.contains::<WallConstructionSite>()
            && !owner_ref.contains::<SoulSpaSite>()
        {
            return Err(format!(
                "WorldMap.buildings[{grid:?}] target {owner:?} has no durable occupancy role"
            ));
        }
    }

    let mut completed_floors = HashMap::new();
    for entity_ref in candidate.iter_entities() {
        let Some(building) = entity_ref.get::<Building>() else {
            continue;
        };
        if building.kind != BuildingType::Floor {
            continue;
        }
        if building.is_provisional {
            return Err(format!(
                "Floor Building {:?} cannot be provisional",
                entity_ref.id()
            ));
        }
        let transform = entity_ref
            .get::<Transform>()
            .ok_or_else(|| format!("completed Floor {:?} has no Transform", entity_ref.id()))?;
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        validate_world_map_grid("floors", grid)?;
        let canonical = WorldMap::grid_to_world(grid.0, grid.1);
        if transform.translation.truncate() != canonical {
            return Err(format!(
                "completed Floor {:?} has non-canonical position {:?}; expected {canonical:?} for grid {grid:?}",
                entity_ref.id(),
                transform.translation.truncate()
            ));
        }
        if let Some(existing) = completed_floors.insert(grid, entity_ref.id()) {
            return Err(format!(
                "completed Floors {existing:?} and {:?} share grid {grid:?}",
                entity_ref.id()
            ));
        }
    }

    let mut mapped_floor_entities = HashSet::new();
    for (&grid, &floor) in &map.floors {
        validate_world_map_grid("floors", grid)?;
        if !mapped_floor_entities.insert(floor) {
            return Err(format!(
                "WorldMap.floors references completed Floor {floor:?} more than once"
            ));
        }
        if completed_floors.get(&grid) != Some(&floor) {
            return Err(format!(
                "WorldMap.floors[{grid:?}] target {floor:?} is not the completed Floor at that grid"
            ));
        }
    }
    let mut legacy_floor_entities = HashSet::new();
    for (&grid, &owner) in &map.buildings {
        if candidate
            .get::<Building>(owner)
            .is_none_or(|building| building.kind != BuildingType::Floor)
        {
            continue;
        }
        if !legacy_floor_entities.insert(owner) {
            return Err(format!(
                "WorldMap.buildings references completed Floor {owner:?} more than once"
            ));
        }
        if completed_floors.get(&grid) != Some(&owner) {
            return Err(format!(
                "WorldMap.buildings[{grid:?}] legacy Floor target {owner:?} is not the completed Floor at that grid"
            ));
        }
    }
    for (&grid, &door) in &map.doors {
        validate_world_map_grid("doors", grid)?;
        let door_ref = candidate
            .get_entity(door)
            .map_err(|_| format!("WorldMap.doors[{grid:?}] references missing Door {door:?}"))?;
        if !door_ref.contains::<Door>()
            || door_ref
                .get::<Building>()
                .is_none_or(|building| building.kind != BuildingType::Door)
        {
            return Err(format!(
                "WorldMap.doors[{grid:?}] target {door:?} is not a completed Door building"
            ));
        }
        if map.buildings.get(&grid) != Some(&door) {
            return Err(format!(
                "WorldMap Door {door:?} at {grid:?} is missing its matching building entry"
            ));
        }
    }
    for (&grid, &stockpile) in &map.stockpiles {
        validate_world_map_grid("stockpiles", grid)?;
        let stockpile_ref = candidate.get_entity(stockpile).map_err(|_| {
            format!("WorldMap.stockpiles[{grid:?}] references missing Stockpile {stockpile:?}")
        })?;
        if !stockpile_ref.contains::<Stockpile>() {
            return Err(format!(
                "WorldMap.stockpiles[{grid:?}] target {stockpile:?} is not a Stockpile"
            ));
        }
    }
    for &grid in map.door_states.keys() {
        validate_world_map_grid("door_states", grid)?;
    }
    for &grid in &map.bridged_tiles {
        validate_world_map_grid("bridged_tiles", grid)?;
    }
    Ok(())
}

pub(super) fn validate_natural_obstacle_positions(candidate: &World) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        let role = if entity.contains::<Tree>() {
            Some("Tree")
        } else if entity.contains::<Rock>() {
            Some("Rock")
        } else {
            None
        };
        let Some(role) = role else {
            continue;
        };
        let position = entity
            .get::<ObstaclePosition>()
            .ok_or_else(|| format!("{role} {:?} has no ObstaclePosition", entity.id()))?;
        if !(0..MAP_WIDTH).contains(&position.0) || !(0..MAP_HEIGHT).contains(&position.1) {
            return Err(format!(
                "{role} {:?} has out-of-bounds ObstaclePosition ({}, {})",
                entity.id(),
                position.0,
                position.1
            ));
        }
    }
    Ok(())
}
