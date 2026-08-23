use super::{super::*, validate_world_map_grid};

pub(in super::super) fn validate_construction_links(candidate: &World) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;
    let mut blueprint_grids: HashMap<Entity, HashSet<(i32, i32)>> = HashMap::new();
    let mut floor_tiles_by_site: HashMap<Entity, HashSet<(i32, i32)>> = HashMap::new();
    let mut floor_states_by_site: HashMap<Entity, Vec<FloorTileState>> = HashMap::new();
    let mut wall_tiles_by_site: HashMap<Entity, HashSet<(i32, i32)>> = HashMap::new();
    let mut wall_states_by_site: HashMap<Entity, Vec<(WallTileState, bool)>> = HashMap::new();
    let mut spawned_wall_owners = HashMap::new();
    let mut wall_occupancy_by_owner: HashMap<Entity, HashSet<(i32, i32)>> = HashMap::new();

    for entity in candidate.iter_entities() {
        if let Some(blueprint) = entity.get::<Blueprint>() {
            if blueprint.occupied_grids.is_empty() {
                return Err(format!(
                    "Blueprint {:?} has an empty occupied_grids footprint",
                    entity.id()
                ));
            }
            let grids = blueprint_grids.entry(entity.id()).or_default();
            for &grid in &blueprint.occupied_grids {
                validate_world_map_grid("Blueprint.occupied_grids", grid)?;
                if !grids.insert(grid) {
                    return Err(format!(
                        "Blueprint {:?} contains duplicate occupied grid {grid:?}",
                        entity.id()
                    ));
                }
                if map.buildings.get(&grid) != Some(&entity.id()) {
                    return Err(format!(
                        "Blueprint {:?} occupied grid {grid:?} is not owned by it in WorldMap.buildings",
                        entity.id()
                    ));
                }
            }
        }

        if let Some(tile) = entity.get::<FloorTileBlueprint>() {
            validate_world_map_grid("FloorTileBlueprint.grid_pos", tile.grid_pos)?;
            let parent = candidate.get_entity(tile.parent_site).map_err(|_| {
                format!(
                    "FloorTileBlueprint {:?} references missing parent site {:?}",
                    entity.id(),
                    tile.parent_site
                )
            })?;
            if !parent.contains::<FloorConstructionSite>() {
                return Err(format!(
                    "FloorTileBlueprint {:?} parent {:?} is not a FloorConstructionSite",
                    entity.id(),
                    tile.parent_site
                ));
            }
            if !floor_tiles_by_site
                .entry(tile.parent_site)
                .or_default()
                .insert(tile.grid_pos)
            {
                return Err(format!(
                    "FloorConstructionSite {:?} has duplicate tile grid {:?}",
                    tile.parent_site, tile.grid_pos
                ));
            }
            floor_states_by_site
                .entry(tile.parent_site)
                .or_default()
                .push(tile.state);
        }

        if let Some(tile) = entity.get::<WallTileBlueprint>() {
            validate_world_map_grid("WallTileBlueprint.grid_pos", tile.grid_pos)?;
            let parent = candidate.get_entity(tile.parent_site).map_err(|_| {
                format!(
                    "WallTileBlueprint {:?} references missing parent site {:?}",
                    entity.id(),
                    tile.parent_site
                )
            })?;
            if !parent.contains::<WallConstructionSite>() {
                return Err(format!(
                    "WallTileBlueprint {:?} parent {:?} is not a WallConstructionSite",
                    entity.id(),
                    tile.parent_site
                ));
            }
            if !wall_tiles_by_site
                .entry(tile.parent_site)
                .or_default()
                .insert(tile.grid_pos)
            {
                return Err(format!(
                    "WallConstructionSite {:?} has duplicate tile grid {:?}",
                    tile.parent_site, tile.grid_pos
                ));
            }
            wall_states_by_site
                .entry(tile.parent_site)
                .or_default()
                .push((tile.state, tile.spawned_wall.is_some()));
            let parent_site = parent
                .get::<WallConstructionSite>()
                .expect("validated WallConstructionSite parent");
            let requires_spawned_wall = matches!(
                tile.state,
                WallTileState::WaitingMud
                    | WallTileState::CoatingReady
                    | WallTileState::Coating { .. }
                    | WallTileState::Complete
            ) || (tile.state == WallTileState::FramedProvisional
                && parent_site.phase == hw_jobs::construction::WallConstructionPhase::Coating);
            if requires_spawned_wall && tile.spawned_wall.is_none() {
                return Err(format!(
                    "WallTileBlueprint {:?} in {:?} has no spawned wall",
                    entity.id(),
                    tile.state
                ));
            }
            if tile.spawned_wall.is_some()
                && matches!(
                    tile.state,
                    WallTileState::WaitingWood
                        | WallTileState::FramingReady
                        | WallTileState::Framing { .. }
                )
            {
                return Err(format!(
                    "WallTileBlueprint {:?} has a spawned wall before framing completed",
                    entity.id()
                ));
            }
            if let Some(wall) = tile.spawned_wall {
                if let Some(previous_tile) = spawned_wall_owners.insert(wall, entity.id()) {
                    return Err(format!(
                        "spawned Wall {wall:?} is shared by WallTileBlueprints {previous_tile:?} and {:?}",
                        entity.id()
                    ));
                }
                let wall_ref = candidate.get_entity(wall).map_err(|_| {
                    format!(
                        "WallTileBlueprint {:?} references missing spawned wall {wall:?}",
                        entity.id()
                    )
                })?;
                if wall_ref
                    .get::<Building>()
                    .is_none_or(|building| building.kind != BuildingType::Wall)
                {
                    return Err(format!(
                        "WallTileBlueprint {:?} spawned wall {wall:?} is not a Wall building",
                        entity.id()
                    ));
                }
                let building = wall_ref
                    .get::<Building>()
                    .expect("validated spawned Wall building");
                if tile.state == WallTileState::Complete {
                    if building.is_provisional || wall_ref.contains::<ProvisionalWall>() {
                        return Err(format!(
                            "completed WallTileBlueprint {:?} spawned wall {wall:?} is still provisional",
                            entity.id()
                        ));
                    }
                } else if !building.is_provisional || !wall_ref.contains::<ProvisionalWall>() {
                    return Err(format!(
                        "WallTileBlueprint {:?} spawned wall {wall:?} is not a provisional Wall",
                        entity.id()
                    ));
                }
            }
            let occupancy_owner = tile.spawned_wall.unwrap_or(tile.parent_site);
            if map.buildings.get(&tile.grid_pos) != Some(&occupancy_owner) {
                return Err(format!(
                    "WallTileBlueprint {:?} grid {:?} has WorldMap owner {:?}, expected {:?}",
                    entity.id(),
                    tile.grid_pos,
                    map.buildings.get(&tile.grid_pos),
                    occupancy_owner
                ));
            }
            wall_occupancy_by_owner
                .entry(occupancy_owner)
                .or_default()
                .insert(tile.grid_pos);
        }
    }

    for site in candidate.iter_entities() {
        if let Some(floor) = site.get::<FloorConstructionSite>() {
            let actual = floor_tiles_by_site.get(&site.id()).map_or(0, HashSet::len);
            if floor.tiles_total == 0 || actual != floor.tiles_total as usize {
                return Err(format!(
                    "FloorConstructionSite {:?} declares {} tile(s), but owns {actual}",
                    site.id(),
                    floor.tiles_total
                ));
            }
            let states = floor_states_by_site
                .get(&site.id())
                .expect("validated floor site tile count");
            let all_reinforced = states.iter().all(|state| {
                matches!(
                    state,
                    FloorTileState::ReinforcedComplete
                        | FloorTileState::WaitingMud
                        | FloorTileState::PouringReady
                        | FloorTileState::Pouring { .. }
                        | FloorTileState::Complete
                )
            });
            let has_pouring_state = states.iter().any(|state| {
                matches!(
                    state,
                    FloorTileState::WaitingMud
                        | FloorTileState::PouringReady
                        | FloorTileState::Pouring { .. }
                        | FloorTileState::Complete
                )
            });
            if (floor.phase == FloorConstructionPhase::Reinforcing
                && has_pouring_state
                && !all_reinforced)
                || (floor.phase == FloorConstructionPhase::Pouring
                    && states.iter().any(|state| {
                        matches!(
                            state,
                            FloorTileState::WaitingBones
                                | FloorTileState::ReinforcingReady
                                | FloorTileState::Reinforcing { .. }
                        )
                    }))
                || (floor.phase == FloorConstructionPhase::Curing
                    && states
                        .iter()
                        .any(|state| *state != FloorTileState::Complete))
            {
                return Err(format!(
                    "FloorConstructionSite {:?} has tile states incompatible with {:?}",
                    site.id(),
                    floor.phase
                ));
            }
        }
        if let Some(wall) = site.get::<WallConstructionSite>() {
            let actual = wall_tiles_by_site.get(&site.id()).map_or(0, HashSet::len);
            if wall.tiles_total == 0 || actual != wall.tiles_total as usize {
                return Err(format!(
                    "WallConstructionSite {:?} declares {} tile(s), but owns {actual}",
                    site.id(),
                    wall.tiles_total
                ));
            }
            let states = wall_states_by_site
                .get(&site.id())
                .expect("validated wall site tile count");
            let all_framed = states.iter().all(|(state, has_wall)| match state {
                WallTileState::FramedProvisional => *has_wall,
                WallTileState::WaitingMud
                | WallTileState::CoatingReady
                | WallTileState::Coating { .. }
                | WallTileState::Complete => true,
                WallTileState::WaitingWood
                | WallTileState::FramingReady
                | WallTileState::Framing { .. } => false,
            });
            let has_coating_state = states.iter().any(|(state, _)| {
                matches!(
                    state,
                    WallTileState::WaitingMud
                        | WallTileState::CoatingReady
                        | WallTileState::Coating { .. }
                        | WallTileState::Complete
                )
            });
            if (wall.phase == WallConstructionPhase::Framing && has_coating_state && !all_framed)
                || (wall.phase == WallConstructionPhase::Coating && !all_framed)
            {
                return Err(format!(
                    "WallConstructionSite {:?} has tile states incompatible with {:?}",
                    site.id(),
                    wall.phase
                ));
            }
        }
    }

    for (&grid, &owner) in &map.buildings {
        if let Some(grids) = blueprint_grids.get(&owner)
            && !grids.contains(&grid)
        {
            return Err(format!(
                "WorldMap.buildings grid {grid:?} points to Blueprint {owner:?} outside occupied_grids"
            ));
        }
        if (candidate.get::<WallConstructionSite>(owner).is_some()
            || spawned_wall_owners.contains_key(&owner))
            && !wall_occupancy_by_owner
                .get(&owner)
                .is_some_and(|grids| grids.contains(&grid))
        {
            return Err(format!(
                "WorldMap.buildings grid {grid:?} points to wall construction owner {owner:?} outside its tile footprint"
            ));
        }
    }
    Ok(())
}
