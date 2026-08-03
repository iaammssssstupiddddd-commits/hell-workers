//! Immutable domain checks for an isolated, entity-remapped load candidate.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, MUD_MIXER_CAPACITY, MUD_MIXER_MUD_CAPACITY};
use hw_core::familiar::{Familiar, FamiliarOperation};
use hw_core::jobs::WorkType;
use hw_core::relationships::{
    CommandedBy, Commanding, LoadedIn, LoadedItems, ManagedBy, ManagedTasks, ParkedAt,
    ParkedWheelbarrows, RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
    StoredIn, StoredItems,
};
use hw_core::soul::{DamnedSoul, DreamState, IdleBehavior, IdleState};
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaSite, SoulSpaTile, YardPowerGrid,
};
use hw_jobs::construction::{
    FloorConstructionPhase, FloorTileBlueprint, FloorTileState, TargetFloorConstructionSite,
    TargetWallConstructionSite, WallConstructionPhase, WallTileBlueprint, WallTileState,
};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer, TargetMixer};
use hw_jobs::{
    Blueprint, Building, BuildingType, Designation, Door, FloorConstructionSite, ObstaclePosition,
    Priority, ProvisionalWall, RestArea, Rock, TargetBlueprint, TargetSoulSpaSite, TaskSlots, Tree,
    TreeVariant, WallConstructionSite,
};
use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportRequest, TransportRequestFixedSource, TransportRequestKind,
};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::{
    BelongsTo, BucketStorage, Inventory, PendingBelongsToBlueprint, ResourceItem, ResourceType,
    Stockpile, Wheelbarrow,
};
use hw_world::{PairedSite, PairedYard, Site, WorldMap, Yard};

use crate::world::map::Tile;

use super::obstacles::DurableNavigationView;

/// Validates every persisted Entity-bearing topology that can be decided in
/// the isolated, remapped candidate. Derived caches may be rebuilt later, but
/// no mutation phase is allowed to discover a missing or mistyped durable
/// endpoint after the live world has already been replaced.
pub(in crate::systems::save) fn validate_durable_topology_candidate(
    candidate: &World,
) -> Result<(), String> {
    validate_world_map_candidate(candidate)?;
    validate_construction_links(candidate)?;
    validate_energy_links(candidate)?;
    validate_zone_links(candidate)?;
    validate_target_links(candidate)?;
    validate_natural_obstacle_positions(candidate)?;
    validate_world_map_tile_anchors(candidate)?;
    Ok(())
}

fn validate_world_map_candidate(candidate: &World) -> Result<(), String> {
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

fn validate_world_map_tile_anchors(candidate: &World) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;
    if let Some(index) = map.tile_entities.iter().position(Option::is_none) {
        return Err(format!(
            "WorldMap.tile_entities[{index}] is missing its Tile anchor"
        ));
    }
    Ok(())
}

fn validate_world_map_grid(field: &str, grid: (i32, i32)) -> Result<(), String> {
    if (0..MAP_WIDTH).contains(&grid.0) && (0..MAP_HEIGHT).contains(&grid.1) {
        Ok(())
    } else {
        Err(format!(
            "WorldMap.{field} contains out-of-bounds grid {grid:?}"
        ))
    }
}

fn validate_natural_obstacle_positions(candidate: &World) -> Result<(), String> {
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

fn validate_construction_links(candidate: &World) -> Result<(), String> {
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

fn validate_energy_links(candidate: &World) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;
    let mut generators_by_grid: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut consumers_by_grid: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut soul_spa_tiles_by_site: HashMap<Entity, HashSet<(i32, i32)>> = HashMap::new();

    for source in candidate.iter_entities() {
        if source.contains::<SoulSpaSite>()
            && (source
                .get::<Building>()
                .is_none_or(|building| building.kind != BuildingType::SoulSpa)
                || !source.contains::<Transform>()
                || !source.contains::<PowerGenerator>())
        {
            return Err(format!(
                "SoulSpaSite {:?} is missing its SoulSpa Building, Transform, or PowerGenerator role",
                source.id()
            ));
        }
        if let Some(owner) = source.get::<YardPowerGrid>() {
            if !source.contains::<PowerGrid>() {
                return Err(format!(
                    "YardPowerGrid source {:?} is not a PowerGrid",
                    source.id()
                ));
            }
            let yard = candidate.get_entity(owner.0).map_err(|_| {
                format!(
                    "PowerGrid {:?} references missing Yard {:?}",
                    source.id(),
                    owner.0
                )
            })?;
            if !yard.contains::<Yard>() {
                return Err(format!(
                    "PowerGrid {:?} YardPowerGrid target {:?} is not a Yard",
                    source.id(),
                    owner.0
                ));
            }
        }
        if let Some(relation) = source.get::<GeneratesFor>() {
            if !source.contains::<PowerGenerator>() {
                return Err(format!(
                    "GeneratesFor source {:?} is not a PowerGenerator",
                    source.id()
                ));
            }
            validate_power_grid_target(candidate, relation.0, "GeneratesFor")?;
            generators_by_grid
                .entry(relation.0)
                .or_default()
                .insert(source.id());
        }
        if let Some(relation) = source.get::<ConsumesFrom>() {
            if !source.contains::<PowerConsumer>() {
                return Err(format!(
                    "ConsumesFrom source {:?} is not a PowerConsumer",
                    source.id()
                ));
            }
            validate_power_grid_target(candidate, relation.0, "ConsumesFrom")?;
            consumers_by_grid
                .entry(relation.0)
                .or_default()
                .insert(source.id());
        }
        if let Some(tile) = source.get::<SoulSpaTile>() {
            let site = candidate.get_entity(tile.parent_site).map_err(|_| {
                format!(
                    "SoulSpaTile {:?} references missing parent site {:?}",
                    source.id(),
                    tile.parent_site
                )
            })?;
            if !site.contains::<SoulSpaSite>() {
                return Err(format!(
                    "SoulSpaTile {:?} parent {:?} is not a SoulSpaSite",
                    source.id(),
                    tile.parent_site
                ));
            }
            validate_world_map_grid("SoulSpaTile.grid_pos", tile.grid_pos)?;
            if map.buildings.get(&tile.grid_pos) != Some(&tile.parent_site) {
                return Err(format!(
                    "SoulSpaTile {:?} grid {:?} is not owned by site {:?} in WorldMap.buildings",
                    source.id(),
                    tile.grid_pos,
                    tile.parent_site
                ));
            }
            if !source.contains::<Transform>() {
                return Err(format!("SoulSpaTile {:?} has no Transform", source.id()));
            }
            if !soul_spa_tiles_by_site
                .entry(tile.parent_site)
                .or_default()
                .insert(tile.grid_pos)
            {
                return Err(format!(
                    "SoulSpaSite {:?} has duplicate tile grid {:?}",
                    tile.parent_site, tile.grid_pos
                ));
            }
        }
    }

    for site in candidate.iter_entities() {
        if site.contains::<SoulSpaSite>() {
            let tile_count = soul_spa_tiles_by_site
                .get(&site.id())
                .map_or(0, HashSet::len);
            if tile_count != 4 {
                return Err(format!(
                    "SoulSpaSite {:?} owns {tile_count} tile(s), expected 4",
                    site.id()
                ));
            }
        }
    }

    for (&grid, &owner) in &map.buildings {
        if candidate.get::<SoulSpaSite>(owner).is_some()
            && !soul_spa_tiles_by_site
                .get(&owner)
                .is_some_and(|grids| grids.contains(&grid))
        {
            return Err(format!(
                "WorldMap.buildings grid {grid:?} points to SoulSpaSite {owner:?} outside its tile footprint"
            ));
        }
    }

    for grid in candidate.iter_entities() {
        validate_relationship_target(
            candidate,
            &grid,
            grid.get::<GridGenerators>()
                .map(|targets| targets.iter().copied()),
            generators_by_grid.get(&grid.id()),
            "GeneratesFor/GridGenerators",
            |source| source.contains::<PowerGenerator>(),
            |source| source.get::<GeneratesFor>().map(|relation| relation.0),
        )?;
        validate_relationship_target(
            candidate,
            &grid,
            grid.get::<GridConsumers>()
                .map(|targets| targets.iter().copied()),
            consumers_by_grid.get(&grid.id()),
            "ConsumesFrom/GridConsumers",
            |source| source.contains::<PowerConsumer>(),
            |source| source.get::<ConsumesFrom>().map(|relation| relation.0),
        )?;
    }
    Ok(())
}

fn validate_power_grid_target(
    candidate: &World,
    target: Entity,
    relation: &str,
) -> Result<(), String> {
    let grid = candidate
        .get_entity(target)
        .map_err(|_| format!("{relation} target {target:?} is missing"))?;
    if grid.contains::<PowerGrid>() {
        Ok(())
    } else {
        Err(format!("{relation} target {target:?} is not a PowerGrid"))
    }
}

fn validate_relationship_target<I, FRole, FTarget>(
    candidate: &World,
    target: &EntityRef<'_>,
    actual: Option<I>,
    expected: Option<&HashSet<Entity>>,
    relation: &str,
    source_has_role: FRole,
    source_target: FTarget,
) -> Result<(), String>
where
    I: Iterator<Item = Entity>,
    FRole: Fn(&EntityRef<'_>) -> bool,
    FTarget: Fn(&EntityRef<'_>) -> Option<Entity>,
{
    let Some(actual) = actual else {
        if expected.is_some_and(|sources| !sources.is_empty()) {
            return Err(format!(
                "{relation} target {:?} is missing its relationship target component",
                target.id()
            ));
        }
        return Ok(());
    };
    if !target.contains::<PowerGrid>() {
        return Err(format!(
            "{relation} target component is attached to non-PowerGrid {:?}",
            target.id()
        ));
    }
    let actual_vec: Vec<_> = actual.collect();
    let actual_set: HashSet<_> = actual_vec.iter().copied().collect();
    if actual_vec.len() != actual_set.len() {
        return Err(format!(
            "{relation} target {:?} contains duplicate sources",
            target.id()
        ));
    }
    if actual_set != expected.cloned().unwrap_or_default() {
        return Err(format!(
            "{relation} is not symmetric for PowerGrid {:?}",
            target.id()
        ));
    }
    for source in actual_set {
        let source_ref = candidate
            .get_entity(source)
            .map_err(|_| format!("{relation} references missing source {source:?}"))?;
        if !source_has_role(&source_ref) || source_target(&source_ref) != Some(target.id()) {
            return Err(format!(
                "{relation} source {source:?} has an invalid role or backlink"
            ));
        }
    }
    Ok(())
}

fn validate_zone_links(candidate: &World) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        if let Some(paired) = entity.get::<PairedYard>() {
            if !entity.contains::<Site>() {
                return Err(format!("PairedYard source {:?} is not a Site", entity.id()));
            }
            let yard = candidate
                .get_entity(paired.0)
                .map_err(|_| format!("PairedYard target {:?} is missing", paired.0))?;
            if !yard.contains::<Yard>()
                || yard
                    .get::<PairedSite>()
                    .is_none_or(|reverse| reverse.0 != entity.id())
            {
                return Err(format!(
                    "PairedYard/PairedSite are not symmetric for Site {:?}",
                    entity.id()
                ));
            }
        }
        if let Some(paired) = entity.get::<PairedSite>() {
            if !entity.contains::<Yard>() {
                return Err(format!("PairedSite source {:?} is not a Yard", entity.id()));
            }
            let site = candidate
                .get_entity(paired.0)
                .map_err(|_| format!("PairedSite target {:?} is missing", paired.0))?;
            if !site.contains::<Site>()
                || site
                    .get::<PairedYard>()
                    .is_none_or(|reverse| reverse.0 != entity.id())
            {
                return Err(format!(
                    "PairedSite/PairedYard are not symmetric for Yard {:?}",
                    entity.id()
                ));
            }
        }
    }
    Ok(())
}

fn validate_target_links(candidate: &World) -> Result<(), String> {
    for source in candidate.iter_entities() {
        validate_target::<Blueprint>(
            candidate,
            &source,
            source.get::<TargetBlueprint>().map(|target| target.0),
            "TargetBlueprint",
        )?;
        validate_target::<FloorConstructionSite>(
            candidate,
            &source,
            source
                .get::<TargetFloorConstructionSite>()
                .map(|target| target.0),
            "TargetFloorConstructionSite",
        )?;
        validate_target::<WallConstructionSite>(
            candidate,
            &source,
            source
                .get::<TargetWallConstructionSite>()
                .map(|target| target.0),
            "TargetWallConstructionSite",
        )?;
        validate_target::<MudMixerStorage>(
            candidate,
            &source,
            source.get::<TargetMixer>().map(|target| target.0),
            "TargetMixer",
        )?;
        validate_target::<SoulSpaSite>(
            candidate,
            &source,
            source.get::<TargetSoulSpaSite>().map(|target| target.0),
            "TargetSoulSpaSite",
        )?;
    }
    Ok(())
}

fn validate_target<TTarget: Component>(
    candidate: &World,
    source: &EntityRef<'_>,
    target: Option<Entity>,
    relation: &str,
) -> Result<(), String> {
    let Some(target) = target else {
        return Ok(());
    };
    let target_ref = candidate.get_entity(target).map_err(|_| {
        format!(
            "{relation} source {:?} references missing target {target:?}",
            source.id()
        )
    })?;
    if !target_ref.contains::<TTarget>() {
        return Err(format!(
            "{relation} source {:?} references a target with the wrong role",
            source.id()
        ));
    }
    Ok(())
}

pub(in crate::systems::save) fn validate_familiar_candidate(
    candidate: &World,
) -> Result<(), String> {
    let mut expected_rosters: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    for soul in candidate.iter_entities() {
        let Some(commanded_by) = soul.get::<CommandedBy>() else {
            continue;
        };
        if !soul.contains::<DamnedSoul>() {
            return Err(format!(
                "CommandedBy source {:?} is not a DamnedSoul",
                soul.id()
            ));
        }
        let familiar = candidate
            .get_entity(commanded_by.0)
            .map_err(|_| format!("CommandedBy target {:?} is missing", commanded_by.0))?;
        if !familiar.contains::<Familiar>() {
            return Err(format!(
                "CommandedBy target {:?} is not a Familiar",
                commanded_by.0
            ));
        }
        expected_rosters
            .entry(commanded_by.0)
            .or_default()
            .insert(soul.id());
    }

    for entity in candidate.iter_entities() {
        let raw_roster_len = entity
            .get::<Commanding>()
            .map_or(0, |roster| roster.iter().count());
        let actual_roster: HashSet<_> = entity
            .get::<Commanding>()
            .into_iter()
            .flat_map(|roster| roster.iter().copied())
            .collect();
        if raw_roster_len != actual_roster.len() {
            return Err(format!(
                "Commanding for Familiar {:?} contains duplicate Souls",
                entity.id()
            ));
        }
        let expected_roster = expected_rosters
            .get(&entity.id())
            .cloned()
            .unwrap_or_default();
        if entity.contains::<Commanding>() && !entity.contains::<Familiar>() {
            return Err(format!(
                "Commanding target {:?} is not a Familiar",
                entity.id()
            ));
        }
        if actual_roster != expected_roster {
            return Err(format!(
                "CommandedBy/Commanding are not symmetric for Familiar {:?}",
                entity.id()
            ));
        }
        if !entity.contains::<Familiar>() {
            continue;
        }
        let Some(operation) = entity.get::<FamiliarOperation>() else {
            continue;
        };
        let roster_len = actual_roster.len();
        if operation.max_controlled_soul < roster_len {
            return Err(format!(
                "FamiliarOperation.max_controlled_soul is {} but the Commanding roster contains {roster_len} Soul(s)",
                operation.max_controlled_soul
            ));
        }
    }
    Ok(())
}

pub(in crate::systems::save) fn validate_task_logistics_candidate(
    candidate: &World,
) -> Result<(), String> {
    validate_durable_owner_links(candidate)?;
    let navigation = DurableNavigationView::from_world(candidate)?;
    let mut inventory_owners = HashSet::new();
    let mut loaded_sources: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut stored_sources: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut mixer_sources: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut home_counts: HashMap<Entity, usize> = HashMap::new();
    let mut parked_sources: HashMap<Entity, HashSet<Entity>> = HashMap::new();

    for owner in candidate.iter_entities() {
        let Some(inventory) = owner.get::<Inventory>() else {
            continue;
        };
        let Some(held) = inventory.0 else {
            continue;
        };
        if !owner.contains::<DamnedSoul>() {
            return Err(format!(
                "Inventory(Some) owner {:?} is not a DamnedSoul",
                owner.id()
            ));
        }
        if !inventory_owners.insert(held) {
            return Err(format!(
                "entity {held:?} is referenced by more than one Soul inventory"
            ));
        }
        let held_ref = candidate
            .get_entity(held)
            .map_err(|_| format!("Soul inventory references missing entity {held:?}"))?;
        if !held_ref.contains::<ResourceItem>() && !held_ref.contains::<Wheelbarrow>() {
            return Err(format!(
                "Soul inventory entity {held:?} is neither a ResourceItem nor a Wheelbarrow"
            ));
        }
        let transform = owner.get::<Transform>().ok_or_else(|| {
            format!(
                "Soul {:?} with Inventory(Some) has no Transform",
                owner.id()
            )
        })?;
        if navigation
            .nearest_walkable_position(transform.translation.truncate())
            .is_none()
        {
            return Err(format!(
                "Soul {:?} has no walkable inventory drop cell",
                owner.id()
            ));
        }
        if held_ref.contains::<ResourceItem>()
            && (held_ref.contains::<LoadedIn>()
                || held_ref.contains::<StoredIn>()
                || held_ref.contains::<StoredByMixer>())
        {
            return Err(format!(
                "ResourceItem {held:?} has both a Soul inventory owner and a durable container owner"
            ));
        }
        if held_ref.contains::<Wheelbarrow>() {
            validate_wheelbarrow_home(candidate, held, &held_ref)?;
        }
    }

    for item in candidate.iter_entities() {
        let loaded_in = item.get::<LoadedIn>();
        let stored_in = item.get::<StoredIn>();
        let stored_by_mixer = item.get::<StoredByMixer>();
        let has_container_owner =
            loaded_in.is_some() || stored_in.is_some() || stored_by_mixer.is_some();
        if !item.contains::<ResourceItem>() && has_container_owner {
            return Err(format!(
                "durable item owner relation source {:?} is not a ResourceItem",
                item.id()
            ));
        }
        if !item.contains::<ResourceItem>() {
            continue;
        }
        let resource = item.get::<ResourceItem>().expect("filtered ResourceItem").0;
        if (resource == hw_core::logistics::ResourceType::Wheelbarrow)
            != item.contains::<Wheelbarrow>()
        {
            return Err(format!(
                "ResourceItem {:?} has an invalid Wheelbarrow marker/type shape",
                item.id()
            ));
        }
        let owner_count = usize::from(inventory_owners.contains(&item.id()))
            + usize::from(loaded_in.is_some())
            + usize::from(stored_in.is_some())
            + usize::from(stored_by_mixer.is_some());
        if owner_count > 1 {
            return Err(format!(
                "ResourceItem {:?} has more than one inventory/container owner",
                item.id()
            ));
        }
        if item.contains::<Wheelbarrow>() && has_container_owner {
            return Err(format!(
                "Wheelbarrow {:?} cannot have a durable container owner",
                item.id()
            ));
        }

        // LoadedIn/LoadedItems survive staging only long enough to carry the
        // remapped wheelbarrow location into RuntimeNormalize. Other items are
        // already ground cargo and need their own final-topology safe cell.
        if !item.contains::<Wheelbarrow>()
            && loaded_in.is_none()
            && stored_in.is_none()
            && stored_by_mixer.is_none()
            && !inventory_owners.contains(&item.id())
        {
            let transform = item
                .get::<Transform>()
                .ok_or_else(|| format!("ground ResourceItem {:?} has no Transform", item.id()))?;
            if navigation
                .nearest_walkable_position(transform.translation.truncate())
                .is_none()
            {
                return Err(format!(
                    "ResourceItem {:?} has no walkable ground normalization cell",
                    item.id()
                ));
            }
        }

        if let Some(loaded_in) = loaded_in {
            if !resource.is_loadable() {
                return Err(format!(
                    "LoadedIn source {:?} has non-loadable resource type {:?}",
                    item.id(),
                    resource
                ));
            }
            let carrier = candidate
                .get_entity(loaded_in.0)
                .map_err(|_| format!("LoadedIn references missing carrier {:?}", loaded_in.0))?;
            if !carrier.contains::<Wheelbarrow>() {
                return Err(format!(
                    "LoadedIn carrier {:?} is not a Wheelbarrow",
                    loaded_in.0
                ));
            }
            loaded_sources
                .entry(loaded_in.0)
                .or_default()
                .insert(item.id());
        }
        if let Some(stored_in) = stored_in {
            let storage = candidate
                .get_entity(stored_in.0)
                .map_err(|_| format!("StoredIn references missing storage {:?}", stored_in.0))?;
            if !storage.contains::<Stockpile>() {
                return Err(format!(
                    "StoredIn target {:?} is not a Stockpile",
                    stored_in.0
                ));
            }
            stored_sources
                .entry(stored_in.0)
                .or_default()
                .insert(item.id());
        }
        if let Some(stored_by_mixer) = stored_by_mixer {
            if resource != hw_core::logistics::ResourceType::StasisMud {
                return Err(format!(
                    "StoredByMixer source {:?} is not StasisMud",
                    item.id()
                ));
            }
            let mixer = candidate.get_entity(stored_by_mixer.0).map_err(|_| {
                format!(
                    "StoredByMixer references missing mixer {:?}",
                    stored_by_mixer.0
                )
            })?;
            if !mixer.contains::<MudMixerStorage>() {
                return Err(format!(
                    "StoredByMixer target {:?} has no MudMixerStorage",
                    stored_by_mixer.0
                ));
            }
            mixer_sources
                .entry(stored_by_mixer.0)
                .or_default()
                .insert(item.id());
        }
    }
    for (carrier, items) in &loaded_sources {
        let capacity = candidate
            .get::<Wheelbarrow>(*carrier)
            .expect("validated Wheelbarrow carrier")
            .capacity;
        if items.len() > capacity {
            return Err(format!(
                "Wheelbarrow {carrier:?} contains {} item(s), exceeding capacity {capacity}",
                items.len()
            ));
        }
    }
    for mixer in candidate.iter_entities() {
        let Some(storage) = mixer.get::<MudMixerStorage>() else {
            continue;
        };
        let expected = mixer_sources.get(&mixer.id()).map_or(0, HashSet::len);
        if storage.mud as usize != expected {
            return Err(format!(
                "MudMixer {:?} stores {} mud unit(s), but owns {expected} StasisMud item(s)",
                mixer.id(),
                storage.mud
            ));
        }
        if storage.sand > MUD_MIXER_CAPACITY
            || storage.rock > MUD_MIXER_CAPACITY
            || storage.mud > MUD_MIXER_MUD_CAPACITY
        {
            return Err(format!(
                "MudMixer {:?} exceeds durable capacity",
                mixer.id()
            ));
        }
    }

    for storage in candidate.iter_entities() {
        let Some(actual_items) = storage.get::<StoredItems>() else {
            if storage.contains::<Stockpile>() && stored_sources.contains_key(&storage.id()) {
                return Err(format!(
                    "Stockpile {:?} is missing StoredItems",
                    storage.id()
                ));
            }
            continue;
        };
        if !storage.contains::<Stockpile>() {
            return Err(format!(
                "StoredItems target {:?} is not a Stockpile",
                storage.id()
            ));
        }
        let actual: HashSet<_> = actual_items.iter().collect();
        if actual.len() != actual_items.len() {
            return Err(format!(
                "StoredItems for Stockpile {:?} contains duplicate sources",
                storage.id()
            ));
        }
        let expected = stored_sources
            .get(&storage.id())
            .cloned()
            .unwrap_or_default();
        if actual != expected {
            return Err(format!(
                "StoredIn/StoredItems are not symmetric for Stockpile {:?}",
                storage.id()
            ));
        }
        let stockpile = storage.get::<Stockpile>().expect("filtered Stockpile");
        let capacity = stockpile.capacity;
        if expected.len() > capacity {
            return Err(format!(
                "Stockpile {:?} contains {} item(s), exceeding capacity {capacity}",
                storage.id(),
                expected.len()
            ));
        }
        if is_bucket_storage_role(candidate, &storage) {
            if !matches!(
                stockpile.resource_type,
                None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
            ) {
                return Err(format!(
                    "BucketStorage {:?} has an incompatible Stockpile resource type",
                    storage.id()
                ));
            }
            let storage_owner = storage.get::<BelongsTo>().map(|owner| owner.0);
            for item_entity in &expected {
                let item = candidate
                    .get::<ResourceItem>(*item_entity)
                    .expect("StoredIn source was validated as a ResourceItem");
                if !matches!(
                    item.0,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                ) {
                    return Err(format!(
                        "BucketStorage {:?} contains non-bucket ResourceItem {:?}",
                        storage.id(),
                        item_entity
                    ));
                }
                if storage_owner.is_none()
                    || candidate
                        .get::<BelongsTo>(*item_entity)
                        .map(|owner| owner.0)
                        != storage_owner
                {
                    return Err(format!(
                        "BucketStorage {:?} and stored bucket {:?} have different durable owners",
                        storage.id(),
                        item_entity
                    ));
                }
            }
        } else {
            for item_entity in &expected {
                let item = candidate
                    .get::<ResourceItem>(*item_entity)
                    .expect("StoredIn source was validated as a ResourceItem");
                if stockpile.resource_type != Some(item.0) {
                    return Err(format!(
                        "Stockpile {:?} resource type does not match stored ResourceItem {:?}",
                        storage.id(),
                        item_entity
                    ));
                }
            }
        }
    }

    for wheelbarrow in candidate.iter_entities() {
        if !wheelbarrow.contains::<Wheelbarrow>() {
            if wheelbarrow.contains::<LoadedItems>() {
                return Err(format!(
                    "LoadedItems target {:?} is not a Wheelbarrow",
                    wheelbarrow.id()
                ));
            }
            continue;
        }
        if wheelbarrow
            .get::<ResourceItem>()
            .is_none_or(|item| item.0 != hw_core::logistics::ResourceType::Wheelbarrow)
        {
            return Err(format!(
                "Wheelbarrow {:?} is missing ResourceItem(Wheelbarrow)",
                wheelbarrow.id()
            ));
        }
        if !inventory_owners.contains(&wheelbarrow.id()) {
            let transform = wheelbarrow
                .get::<Transform>()
                .ok_or_else(|| format!("Wheelbarrow {:?} has no Transform", wheelbarrow.id()))?;
            if navigation
                .nearest_walkable_position(transform.translation.truncate())
                .is_none()
            {
                return Err(format!(
                    "Wheelbarrow {:?} has no walkable normalization cell",
                    wheelbarrow.id()
                ));
            }
        }
        let home = validate_wheelbarrow_home(candidate, wheelbarrow.id(), &wheelbarrow)?;
        *home_counts.entry(home).or_default() += 1;
        if let Some(parked_at) = wheelbarrow.get::<ParkedAt>() {
            validate_parking_target(candidate, parked_at.0)?;
            if parked_at.0 != home {
                return Err(format!(
                    "Wheelbarrow {:?} is parked at {:?}, but its durable home is {home:?}",
                    wheelbarrow.id(),
                    parked_at.0
                ));
            }
            parked_sources
                .entry(parked_at.0)
                .or_default()
                .insert(wheelbarrow.id());
        }

        let actual_loaded = wheelbarrow.get::<LoadedItems>();
        let actual_loaded_set: HashSet<_> = actual_loaded
            .into_iter()
            .flat_map(|items| items.iter())
            .collect();
        if actual_loaded.is_some_and(|items| items.len() != actual_loaded_set.len()) {
            return Err(format!(
                "LoadedItems for Wheelbarrow {:?} contains duplicate sources",
                wheelbarrow.id()
            ));
        }
        let expected_loaded = loaded_sources
            .get(&wheelbarrow.id())
            .cloned()
            .unwrap_or_default();
        if actual_loaded_set != expected_loaded {
            return Err(format!(
                "LoadedIn/LoadedItems are not symmetric for Wheelbarrow {:?}",
                wheelbarrow.id()
            ));
        }
    }
    for (parking, count) in home_counts {
        let capacity = candidate
            .get::<WheelbarrowParking>(parking)
            .expect("validated WheelbarrowParking target")
            .capacity;
        if count > capacity {
            return Err(format!(
                "WheelbarrowParking {parking:?} contains {count} wheelbarrow(s), exceeding capacity {capacity}"
            ));
        }
    }
    for parking in candidate.iter_entities() {
        if !parking.contains::<WheelbarrowParking>() {
            if parking.contains::<ParkedWheelbarrows>() {
                return Err(format!(
                    "ParkedWheelbarrows target {:?} is not WheelbarrowParking",
                    parking.id()
                ));
            }
            continue;
        }
        let actual: HashSet<_> = parking
            .get::<ParkedWheelbarrows>()
            .into_iter()
            .flat_map(|wheelbarrows| wheelbarrows.iter())
            .collect();
        let raw_len = parking
            .get::<ParkedWheelbarrows>()
            .map_or(0, |wheelbarrows| wheelbarrows.iter().count());
        if actual.len() != raw_len {
            return Err(format!(
                "ParkedWheelbarrows for parking {:?} contains duplicate sources",
                parking.id()
            ));
        }
        let expected = parked_sources
            .get(&parking.id())
            .cloned()
            .unwrap_or_default();
        if actual != expected {
            return Err(format!(
                "ParkedAt/ParkedWheelbarrows are not symmetric for parking {:?}",
                parking.id()
            ));
        }
    }

    for task in candidate.iter_entities() {
        let Some(managed_by) = task.get::<ManagedBy>() else {
            continue;
        };
        let manager = candidate
            .get_entity(managed_by.0)
            .map_err(|_| format!("ManagedBy references missing Familiar {:?}", managed_by.0))?;
        if !manager.contains::<Familiar>() && !manager.contains::<Yard>() {
            return Err(format!(
                "ManagedBy target {:?} is neither a Familiar nor a Yard",
                managed_by.0
            ));
        }
        if !manager
            .get::<ManagedTasks>()
            .is_some_and(|tasks| tasks.contains(task.id()))
        {
            return Err(format!(
                "ManagedBy/ManagedTasks are not symmetric for task {:?}",
                task.id()
            ));
        }
    }
    for manager in candidate.iter_entities() {
        let Some(tasks) = manager.get::<ManagedTasks>() else {
            continue;
        };
        if !manager.contains::<Familiar>() && !manager.contains::<Yard>() {
            return Err(format!(
                "ManagedTasks target {:?} is neither a Familiar nor a Yard",
                manager.id()
            ));
        }
        let unique: HashSet<_> = tasks.iter().collect();
        if unique.len() != tasks.len() {
            return Err(format!(
                "ManagedTasks for owner {:?} contains duplicate sources",
                manager.id()
            ));
        }
        for task in tasks.iter() {
            if candidate
                .get::<ManagedBy>(*task)
                .is_none_or(|managed_by| managed_by.0 != manager.id())
            {
                return Err(format!(
                    "ManagedTasks contains task {task:?} without the matching ManagedBy source"
                ));
            }
        }
    }

    let mut pinned_source_owners: HashMap<Entity, usize> = HashMap::new();
    for entity in candidate.iter_entities() {
        if let Some(request) = entity.get::<TransportRequest>() {
            validate_transport_request_candidate(candidate, &entity, request)?;
            if let Some(source) = entity.get::<TransportRequestFixedSource>() {
                *pinned_source_owners.entry(source.0).or_default() += 1;
            }
        }
    }
    validate_manual_request_ownership(candidate, &pinned_source_owners)?;
    validate_rest_relationships(candidate)?;

    Ok(())
}

fn validate_durable_owner_links(candidate: &World) -> Result<(), String> {
    for source in candidate.iter_entities() {
        let belongs_to = source.get::<BelongsTo>();
        let pending = source.get::<PendingBelongsToBlueprint>();
        if belongs_to.is_some() && pending.is_some() {
            return Err(format!(
                "entity {:?} has both BelongsTo and PendingBelongsToBlueprint",
                source.id()
            ));
        }

        if let Some(owner) = belongs_to {
            let target = candidate.get_entity(owner.0).map_err(|_| {
                format!(
                    "BelongsTo source {:?} references missing owner {:?}",
                    source.id(),
                    owner.0
                )
            })?;
            if source.contains::<Wheelbarrow>() {
                if !target.contains::<WheelbarrowParking>() {
                    return Err(format!(
                        "Wheelbarrow {:?} BelongsTo target {:?} is not WheelbarrowParking",
                        source.id(),
                        owner.0
                    ));
                }
            } else if source.contains::<Stockpile>() {
                if !target.contains::<Yard>() && !is_tank_building(&target) {
                    return Err(format!(
                        "Stockpile {:?} BelongsTo target {:?} is neither Yard nor Tank",
                        source.id(),
                        owner.0
                    ));
                }
            } else if let Some(item) = source.get::<ResourceItem>() {
                let valid_owner = if matches!(
                    item.0,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                ) {
                    is_tank_building(&target)
                } else {
                    target.contains::<Familiar>() || target.contains::<Yard>()
                };
                if !valid_owner {
                    return Err(format!(
                        "ResourceItem {:?} has an incompatible BelongsTo owner {:?}",
                        source.id(),
                        owner.0
                    ));
                }
            } else {
                return Err(format!(
                    "BelongsTo source {:?} has no supported durable owner role",
                    source.id()
                ));
            }
        }

        if let Some(pending) = pending {
            let blueprint = candidate.get_entity(pending.0).map_err(|_| {
                format!(
                    "PendingBelongsToBlueprint source {:?} references missing Blueprint {:?}",
                    source.id(),
                    pending.0
                )
            })?;
            if !source.contains::<Stockpile>() || !is_tank_blueprint(&blueprint) {
                return Err(format!(
                    "PendingBelongsToBlueprint source {:?} is not a Tank companion Stockpile",
                    source.id()
                ));
            }
        }
    }
    Ok(())
}

fn is_tank_building(entity: &EntityRef<'_>) -> bool {
    entity
        .get::<Building>()
        .is_some_and(|building| building.kind == BuildingType::Tank)
}

fn is_tank_blueprint(entity: &EntityRef<'_>) -> bool {
    entity
        .get::<Blueprint>()
        .is_some_and(|blueprint| blueprint.kind == BuildingType::Tank)
}

fn is_bucket_storage_role(candidate: &World, storage: &EntityRef<'_>) -> bool {
    storage.contains::<BucketStorage>()
        || storage
            .get::<BelongsTo>()
            .and_then(|owner| candidate.get_entity(owner.0).ok())
            .is_some_and(|owner| is_tank_building(&owner))
        || storage
            .get::<PendingBelongsToBlueprint>()
            .and_then(|owner| candidate.get_entity(owner.0).ok())
            .is_some_and(|owner| is_tank_blueprint(&owner))
}

pub(super) fn validate_shell_candidate(candidate: &World) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        let spatial_root = if entity.contains::<DamnedSoul>() {
            Some("DamnedSoul")
        } else if entity.contains::<Familiar>() {
            Some("Familiar")
        } else if entity.contains::<Building>() {
            Some("Building")
        } else if entity.contains::<Blueprint>() {
            Some("Blueprint")
        } else if entity.contains::<FloorConstructionSite>() {
            Some("FloorConstructionSite")
        } else if entity.contains::<FloorTileBlueprint>() {
            Some("FloorTileBlueprint")
        } else if entity.contains::<WallConstructionSite>() {
            Some("WallConstructionSite")
        } else if entity.contains::<WallTileBlueprint>() {
            Some("WallTileBlueprint")
        } else if entity.contains::<Tree>() {
            Some("Tree")
        } else if entity.contains::<Rock>() {
            Some("Rock")
        } else if entity.contains::<ResourceItem>() {
            Some("ResourceItem")
        } else if entity.contains::<Stockpile>() {
            Some("Stockpile")
        } else {
            None
        };
        if let Some(label) = spatial_root
            && !entity.contains::<Transform>()
        {
            return Err(format!("{label} {:?} has no Transform", entity.id()));
        }
        if entity.contains::<DamnedSoul>()
            && (!entity.contains::<IdleState>()
                || !entity.contains::<DreamState>()
                || !entity.contains::<Inventory>())
        {
            return Err(format!(
                "DamnedSoul {:?} is missing IdleState, DreamState, or Inventory",
                entity.id()
            ));
        }
        if entity.contains::<Tree>() && !entity.contains::<TreeVariant>() {
            return Err(format!("Tree {:?} has no TreeVariant", entity.id()));
        }
    }
    Ok(())
}

fn validate_rest_relationships(candidate: &World) -> Result<(), String> {
    let mut expected_occupants: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut expected_reservations: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    for soul in candidate.iter_entities() {
        if soul.contains::<RestingIn>() && soul.contains::<RestAreaReservedFor>() {
            return Err(format!(
                "DamnedSoul {:?} is both resting and reserved",
                soul.id()
            ));
        }
        if let Some(resting_in) = soul.get::<RestingIn>() {
            validate_rest_source_target(candidate, &soul, resting_in.0, "RestingIn")?;
            if soul
                .get::<IdleState>()
                .is_none_or(|idle| idle.behavior != IdleBehavior::Resting)
            {
                return Err(format!(
                    "RestingIn Soul {:?} is not in IdleBehavior::Resting",
                    soul.id()
                ));
            }
            expected_occupants
                .entry(resting_in.0)
                .or_default()
                .insert(soul.id());
        }
        if let Some(reserved_for) = soul.get::<RestAreaReservedFor>() {
            validate_rest_source_target(candidate, &soul, reserved_for.0, "RestAreaReservedFor")?;
            if soul
                .get::<IdleState>()
                .is_none_or(|idle| idle.behavior != IdleBehavior::GoingToRest)
            {
                return Err(format!(
                    "reserved Soul {:?} is not in IdleBehavior::GoingToRest",
                    soul.id()
                ));
            }
            expected_reservations
                .entry(reserved_for.0)
                .or_default()
                .insert(soul.id());
        }
    }
    for rest_area in candidate.iter_entities() {
        let occupant_len = rest_area
            .get::<RestAreaOccupants>()
            .map_or(0, |occupants| occupants.iter().count());
        let reservation_len = rest_area
            .get::<RestAreaReservations>()
            .map_or(0, |reservations| reservations.iter().count());
        let actual_occupants: HashSet<_> = rest_area
            .get::<RestAreaOccupants>()
            .into_iter()
            .flat_map(|occupants| occupants.iter().copied())
            .collect();
        let actual_reservations: HashSet<_> = rest_area
            .get::<RestAreaReservations>()
            .into_iter()
            .flat_map(|reservations| reservations.iter().copied())
            .collect();
        if occupant_len != actual_occupants.len() || reservation_len != actual_reservations.len() {
            return Err(format!(
                "rest relationship target {:?} contains duplicate Souls",
                rest_area.id()
            ));
        }
        if (rest_area.contains::<RestAreaOccupants>()
            || rest_area.contains::<RestAreaReservations>())
            && !rest_area.contains::<RestArea>()
        {
            return Err(format!(
                "rest relationship target {:?} is not a RestArea",
                rest_area.id()
            ));
        }
        if actual_occupants
            != expected_occupants
                .get(&rest_area.id())
                .cloned()
                .unwrap_or_default()
        {
            return Err(format!(
                "RestingIn/RestAreaOccupants are not symmetric for {:?}",
                rest_area.id()
            ));
        }
        if actual_reservations
            != expected_reservations
                .get(&rest_area.id())
                .cloned()
                .unwrap_or_default()
        {
            return Err(format!(
                "RestAreaReservedFor/RestAreaReservations are not symmetric for {:?}",
                rest_area.id()
            ));
        }
        if let Some(area) = rest_area.get::<RestArea>()
            && actual_occupants.len() + actual_reservations.len() > area.capacity
        {
            return Err(format!(
                "RestArea {:?} exceeds occupant/reservation capacity",
                rest_area.id()
            ));
        }
    }
    Ok(())
}

fn validate_rest_source_target(
    candidate: &World,
    source: &EntityRef<'_>,
    target: Entity,
    relation: &str,
) -> Result<(), String> {
    if !source.contains::<DamnedSoul>() {
        return Err(format!(
            "{relation} source {:?} is not a DamnedSoul",
            source.id()
        ));
    }
    let target_ref = candidate
        .get_entity(target)
        .map_err(|_| format!("{relation} target {target:?} is missing"))?;
    if !target_ref.contains::<RestArea>() {
        return Err(format!("{relation} target {target:?} is not a RestArea"));
    }
    Ok(())
}

fn validate_transport_request_candidate(
    candidate: &World,
    entity: &EntityRef<'_>,
    request: &TransportRequest,
) -> Result<(), String> {
    macro_rules! require_component {
        ($type:ty) => {
            if !entity.contains::<$type>() {
                return Err(format!(
                    "TransportRequest {:?} is missing {}",
                    entity.id(),
                    std::any::type_name::<$type>()
                ));
            }
        };
    }

    require_component!(Transform);
    require_component!(ManagedBy);
    require_component!(TransportDemand);
    require_component!(TransportPolicy);

    // Producers disable a zero-demand request by removing this exact trio and
    // keep the request entity for in-flight accounting. Both the fully active
    // and fully disabled shapes are valid; a partial trio is corruption.
    let active_shape = [
        entity.contains::<Designation>(),
        entity.contains::<TaskSlots>(),
        entity.contains::<Priority>(),
    ];
    if active_shape.iter().any(|present| *present) && active_shape.iter().any(|present| !*present) {
        return Err(format!(
            "TransportRequest {:?} has a partial active task shape",
            entity.id()
        ));
    }
    let demand = entity
        .get::<TransportDemand>()
        .expect("required TransportDemand was checked above");
    if active_shape.iter().all(|present| *present) {
        let slots = entity
            .get::<TaskSlots>()
            .expect("active task shape includes TaskSlots");
        if slots.max == 0 || slots.max != demand.desired_slots {
            return Err(format!(
                "active TransportRequest {:?} has inconsistent TaskSlots/TransportDemand",
                entity.id()
            ));
        }
    }
    candidate
        .get_entity(request.anchor)
        .map_err(|_| format!("TransportRequest anchor {:?} is missing", request.anchor))?;
    let issuer = candidate
        .get_entity(request.issued_by)
        .map_err(|_| format!("TransportRequest issuer {:?} is missing", request.issued_by))?;
    if !issuer.contains::<Familiar>() && !issuer.contains::<Yard>() {
        return Err(format!(
            "TransportRequest issuer {:?} is neither a Familiar nor a Yard",
            request.issued_by
        ));
    }
    if entity
        .get::<ManagedBy>()
        .is_none_or(|managed_by| managed_by.0 != request.issued_by)
    {
        return Err(format!(
            "TransportRequest {:?} issuer and ManagedBy owner differ",
            entity.id()
        ));
    }
    for stockpile in &request.stockpile_group {
        let target = candidate.get_entity(*stockpile).map_err(|_| {
            format!("TransportRequest stockpile-group target {stockpile:?} is missing")
        })?;
        if !target.contains::<Stockpile>() {
            return Err(format!(
                "TransportRequest stockpile-group target {stockpile:?} is not a Stockpile"
            ));
        }
    }

    validate_request_target_shape(candidate, entity, request)?;

    let manual = entity.contains::<ManualTransportRequest>();
    let fixed_source = entity.get::<TransportRequestFixedSource>();
    if manual != fixed_source.is_some() {
        return Err(format!(
            "TransportRequest {:?} must pair ManualTransportRequest with exactly one fixed source",
            entity.id()
        ));
    }
    if manual
        && (!active_shape.iter().all(|present| *present)
            || request.kind != TransportRequestKind::DepositToStockpile
            || !request.stockpile_group.is_empty()
            || !issuer.contains::<Familiar>()
            || entity.get::<TaskSlots>().is_none_or(|slots| slots.max != 1)
            || demand.desired_slots != 1)
    {
        return Err(format!(
            "manual TransportRequest {:?} has a non-canonical active shape",
            entity.id()
        ));
    }
    if let Some(fixed_source) = fixed_source {
        let source = candidate.get_entity(fixed_source.0).map_err(|_| {
            format!(
                "manual TransportRequest fixed source {:?} is missing",
                fixed_source.0
            )
        })?;
        if !source.contains::<ResourceItem>() || !source.contains::<ManualHaulPinnedSource>() {
            return Err(format!(
                "manual TransportRequest fixed source {:?} is not a pinned ResourceItem",
                fixed_source.0
            ));
        }
        if source
            .get::<ResourceItem>()
            .is_none_or(|item| item.0 != request.resource_type)
        {
            return Err(format!(
                "manual TransportRequest {:?} resource type differs from its fixed source",
                entity.id()
            ));
        }
    }
    Ok(())
}

fn validate_request_target_shape(
    candidate: &World,
    entity: &EntityRef<'_>,
    request: &TransportRequest,
) -> Result<(), String> {
    let blueprint_target = entity.get::<TargetBlueprint>().map(|target| target.0);
    let floor_target = entity.get::<TargetFloorConstructionSite>();
    let wall_target = entity.get::<TargetWallConstructionSite>();
    let mixer_target = entity.get::<TargetMixer>().map(|target| target.0);
    let soul_spa_target = entity.get::<TargetSoulSpaSite>().map(|target| target.0);

    let expected_marker = match request.kind {
        TransportRequestKind::DeliverToBlueprint => Some("blueprint"),
        TransportRequestKind::DeliverToFloorConstruction => Some("floor"),
        TransportRequestKind::DeliverToWallConstruction => Some("wall"),
        TransportRequestKind::DeliverToMixerSolid | TransportRequestKind::DeliverWaterToMixer => {
            Some("mixer")
        }
        TransportRequestKind::DeliverToSoulSpa => Some("soul-spa"),
        _ => None,
    };
    for (label, target) in [
        ("blueprint", blueprint_target),
        ("floor", floor_target.map(|target| target.0)),
        ("wall", wall_target.map(|target| target.0)),
        ("mixer", mixer_target),
        ("soul-spa", soul_spa_target),
    ] {
        if let Some(target) = target
            && (expected_marker != Some(label) || target != request.anchor)
        {
            return Err(format!(
                "TransportRequest {:?} has an invalid {label} target marker",
                entity.id()
            ));
        }
    }

    let anchor = candidate
        .get_entity(request.anchor)
        .map_err(|_| format!("TransportRequest anchor {:?} is missing", request.anchor))?;
    let (anchor_valid, expected_work, resource_valid) = match request.kind {
        TransportRequestKind::DepositToStockpile => {
            (anchor.contains::<Stockpile>(), WorkType::Haul, true)
        }
        TransportRequestKind::DeliverToBlueprint => {
            (anchor.contains::<Blueprint>(), WorkType::Haul, true)
        }
        TransportRequestKind::DeliverToFloorConstruction => (
            anchor.contains::<FloorConstructionSite>(),
            WorkType::Haul,
            matches!(
                request.resource_type,
                hw_core::logistics::ResourceType::Bone
                    | hw_core::logistics::ResourceType::StasisMud
            ),
        ),
        TransportRequestKind::DeliverToWallConstruction => (
            anchor.contains::<WallConstructionSite>(),
            WorkType::Haul,
            matches!(
                request.resource_type,
                hw_core::logistics::ResourceType::Wood
                    | hw_core::logistics::ResourceType::StasisMud
            ),
        ),
        TransportRequestKind::DeliverToProvisionalWall => (
            anchor.contains::<Building>() && anchor.contains::<ProvisionalWall>(),
            WorkType::Haul,
            request.resource_type == hw_core::logistics::ResourceType::StasisMud,
        ),
        TransportRequestKind::DeliverToMixerSolid => (
            anchor.contains::<MudMixerStorage>(),
            WorkType::HaulToMixer,
            matches!(
                request.resource_type,
                hw_core::logistics::ResourceType::Sand | hw_core::logistics::ResourceType::Rock
            ),
        ),
        TransportRequestKind::DeliverWaterToMixer => (
            anchor.contains::<MudMixerStorage>(),
            WorkType::HaulWaterToMixer,
            request.resource_type == hw_core::logistics::ResourceType::Water,
        ),
        TransportRequestKind::GatherWaterToTank => (
            anchor.get::<Stockpile>().is_some_and(|stockpile| {
                stockpile.resource_type == Some(hw_core::logistics::ResourceType::Water)
            }),
            WorkType::GatherWater,
            request.resource_type == hw_core::logistics::ResourceType::Water,
        ),
        TransportRequestKind::ReturnBucket => (
            anchor.get::<Stockpile>().is_some_and(|stockpile| {
                stockpile.resource_type == Some(hw_core::logistics::ResourceType::Water)
            }),
            WorkType::Haul,
            request.resource_type == hw_core::logistics::ResourceType::BucketEmpty,
        ),
        TransportRequestKind::ReturnWheelbarrow | TransportRequestKind::BatchWheelbarrow => (
            anchor.contains::<Wheelbarrow>(),
            WorkType::WheelbarrowHaul,
            request.resource_type == hw_core::logistics::ResourceType::Wheelbarrow,
        ),
        TransportRequestKind::ConsolidateStockpile => {
            (anchor.contains::<Stockpile>(), WorkType::Haul, true)
        }
        TransportRequestKind::DeliverToSoulSpa => (
            anchor.contains::<SoulSpaSite>(),
            WorkType::Haul,
            request.resource_type == hw_core::logistics::ResourceType::Bone,
        ),
    };
    if !anchor_valid || !resource_valid {
        return Err(format!(
            "TransportRequest {:?} has an invalid anchor/resource shape for {:?}",
            entity.id(),
            request.kind
        ));
    }
    if let Some(designation) = entity.get::<Designation>()
        && designation.work_type != expected_work
    {
        return Err(format!(
            "TransportRequest {:?} has the wrong active WorkType",
            entity.id()
        ));
    }

    // Marker-less legacy payloads are accepted because kind+anchor is a complete
    // source of truth. DurableNormalize restores the marker before producers run.
    match request.kind {
        TransportRequestKind::DeliverToFloorConstruction if wall_target.is_some() => {
            return Err(format!(
                "floor TransportRequest {:?} also has a wall target",
                entity.id()
            ));
        }
        TransportRequestKind::DeliverToWallConstruction if floor_target.is_some() => {
            return Err(format!(
                "wall TransportRequest {:?} also has a floor target",
                entity.id()
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_manual_request_ownership(
    candidate: &World,
    owners: &HashMap<Entity, usize>,
) -> Result<(), String> {
    for entity in candidate.iter_entities() {
        if (entity.contains::<ManualTransportRequest>()
            || entity.contains::<TransportRequestFixedSource>())
            && !entity.contains::<TransportRequest>()
        {
            return Err(format!(
                "manual request component is attached to non-request {:?}",
                entity.id()
            ));
        }
        if entity.contains::<ManualHaulPinnedSource>() {
            if !entity.contains::<ResourceItem>() {
                return Err(format!(
                    "ManualHaulPinnedSource {:?} is not a ResourceItem",
                    entity.id()
                ));
            }
            if owners.get(&entity.id()).copied().unwrap_or(0) != 1 {
                return Err(format!(
                    "ManualHaulPinnedSource {:?} must have exactly one request owner",
                    entity.id()
                ));
            }
        }
    }
    Ok(())
}

fn validate_wheelbarrow_home(
    candidate: &World,
    wheelbarrow: Entity,
    wheelbarrow_ref: &EntityRef<'_>,
) -> Result<Entity, String> {
    let belongs_to = wheelbarrow_ref
        .get::<BelongsTo>()
        .ok_or_else(|| format!("Wheelbarrow {wheelbarrow:?} has no durable BelongsTo home"))?;
    validate_parking_target(candidate, belongs_to.0)?;
    Ok(belongs_to.0)
}

fn validate_parking_target(candidate: &World, parking: Entity) -> Result<(), String> {
    let parking_ref = candidate
        .get_entity(parking)
        .map_err(|_| format!("wheelbarrow parking reference {parking:?} is missing"))?;
    if parking_ref.contains::<WheelbarrowParking>() {
        Ok(())
    } else {
        Err(format!(
            "wheelbarrow parking reference {parking:?} is not a WheelbarrowParking"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::relationships::{CommandedBy, LoadedItems, ManagedBy, ParkedWheelbarrows};
    use hw_world::WorldMap;

    fn candidate_world() -> World {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());
        world
    }

    #[test]
    fn familiar_roster_validation_is_immutable_and_rejects_saved_overflow() {
        let mut world = candidate_world();
        let familiar = world
            .spawn((
                Familiar::default(),
                FamiliarOperation {
                    max_controlled_soul: 1,
                    ..default()
                },
            ))
            .id();
        world.spawn((DamnedSoul::default(), CommandedBy(familiar)));
        world.spawn((DamnedSoul::default(), CommandedBy(familiar)));
        world.flush();

        let error = validate_familiar_candidate(&world).unwrap_err();

        assert!(error.contains("roster contains 2"));
        assert_eq!(world.get::<Commanding>(familiar).unwrap().iter().count(), 2);
    }

    #[test]
    fn durable_topology_validation_rejects_invalid_world_map_shape() {
        let mut world = candidate_world();
        world.resource_mut::<WorldMap>().obstacles.pop();

        assert!(
            validate_durable_topology_candidate(&world)
                .unwrap_err()
                .contains("WorldMap.obstacles has length")
        );
    }

    #[test]
    fn durable_topology_validation_requires_every_tile_anchor() {
        let world = candidate_world();

        assert!(
            validate_durable_topology_candidate(&world)
                .unwrap_err()
                .contains("is missing its Tile anchor")
        );
    }

    #[test]
    fn durable_topology_validation_accepts_complete_unique_tile_anchors() {
        let mut world = candidate_world();
        let tile_count = (MAP_WIDTH * MAP_HEIGHT) as usize;
        let anchors: Vec<_> = (0..tile_count)
            .map(|_| world.spawn((Tile, Transform::default())).id())
            .collect();
        world.resource_mut::<WorldMap>().tile_entities = anchors.into_iter().map(Some).collect();

        validate_durable_topology_candidate(&world).unwrap();
    }

    #[test]
    fn durable_topology_validation_rejects_blueprint_and_wall_map_mismatches() {
        let mut blueprint_world = candidate_world();
        blueprint_world.spawn((
            Blueprint::new(BuildingType::Tank, vec![(4, 5)]),
            Transform::default(),
        ));
        assert!(
            validate_durable_topology_candidate(&blueprint_world)
                .unwrap_err()
                .contains("is not owned by it in WorldMap.buildings")
        );

        let mut wall_world = candidate_world();
        let site = wall_world
            .spawn(WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                1,
            ))
            .id();
        wall_world.spawn(WallTileBlueprint::new(site, (6, 7)));
        assert!(
            validate_durable_topology_candidate(&wall_world)
                .unwrap_err()
                .contains("has WorldMap owner")
        );
    }

    #[test]
    fn construction_validation_accepts_bridge_and_unspawned_wall_boundary() {
        let mut world = candidate_world();
        let bridge = world
            .spawn(Blueprint::new(BuildingType::Bridge, vec![(1, 2), (2, 2)]))
            .id();
        let wall_site = world
            .spawn(WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                1,
            ))
            .id();
        let mut wall_tile = WallTileBlueprint::new(wall_site, (3, 3));
        wall_tile.state = WallTileState::FramedProvisional;
        world.spawn(wall_tile);
        {
            let mut map = world.resource_mut::<WorldMap>();
            map.set_building((1, 2), bridge);
            map.set_building((2, 2), bridge);
            map.set_building((3, 3), wall_site);
        }

        validate_construction_links(&world).unwrap();
        let map = world.resource::<WorldMap>();
        assert!(!map.has_raw_obstacle(1, 2));
        assert!(!map.bridged_tiles.contains(&(1, 2)));
    }

    #[test]
    fn construction_validation_rejects_reverse_footprint_extras() {
        let mut blueprint_world = candidate_world();
        let blueprint = blueprint_world
            .spawn(Blueprint::new(BuildingType::Tank, vec![(1, 1)]))
            .id();
        {
            let mut map = blueprint_world.resource_mut::<WorldMap>();
            map.set_building((1, 1), blueprint);
            map.set_building((2, 1), blueprint);
        }
        assert!(
            validate_construction_links(&blueprint_world)
                .unwrap_err()
                .contains("outside occupied_grids")
        );

        let mut wall_world = candidate_world();
        let site = wall_world
            .spawn(WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                1,
            ))
            .id();
        wall_world.spawn(WallTileBlueprint::new(site, (4, 4)));
        {
            let mut map = wall_world.resource_mut::<WorldMap>();
            map.set_building((4, 4), site);
            map.set_building((5, 4), site);
        }
        assert!(
            validate_construction_links(&wall_world)
                .unwrap_err()
                .contains("outside its tile footprint")
        );
    }

    #[test]
    fn durable_topology_validation_rejects_stalled_construction_phase_mixtures() {
        let mut floor_world = candidate_world();
        let floor_site = floor_world
            .spawn(FloorConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                2,
            ))
            .id();
        let mut early_floor = FloorTileBlueprint::new(floor_site, (1, 1));
        early_floor.state = FloorTileState::WaitingBones;
        floor_world.spawn(early_floor);
        let mut advanced_floor = FloorTileBlueprint::new(floor_site, (2, 1));
        advanced_floor.state = FloorTileState::WaitingMud;
        floor_world.spawn(advanced_floor);
        assert!(
            validate_durable_topology_candidate(&floor_world)
                .unwrap_err()
                .contains("tile states incompatible with Reinforcing")
        );

        let mut wall_world = candidate_world();
        let wall_site = wall_world
            .spawn(WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                2,
            ))
            .id();
        let provisional_wall = wall_world
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
            ))
            .id();
        let mut advanced_wall = WallTileBlueprint::new(wall_site, (3, 3));
        advanced_wall.state = WallTileState::WaitingMud;
        advanced_wall.spawned_wall = Some(provisional_wall);
        wall_world.spawn(advanced_wall);
        wall_world.spawn(WallTileBlueprint::new(wall_site, (4, 3)));
        {
            let mut map = wall_world.resource_mut::<WorldMap>();
            map.set_building((3, 3), provisional_wall);
            map.set_building((4, 3), wall_site);
        }
        assert!(
            validate_durable_topology_candidate(&wall_world)
                .unwrap_err()
                .contains("tile states incompatible with Framing")
        );
    }

    #[test]
    fn durable_topology_validation_rejects_non_provisional_spawned_wall() {
        let mut world = candidate_world();
        let site = world
            .spawn(WallConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                Vec2::ZERO,
                1,
            ))
            .id();
        let permanent_wall = world
            .spawn(Building {
                kind: BuildingType::Wall,
                is_provisional: false,
            })
            .id();
        let mut tile = WallTileBlueprint::new(site, (5, 5));
        tile.state = WallTileState::FramedProvisional;
        tile.spawned_wall = Some(permanent_wall);
        world.spawn(tile);
        world
            .resource_mut::<WorldMap>()
            .set_building((5, 5), permanent_wall);

        assert!(
            validate_durable_topology_candidate(&world)
                .unwrap_err()
                .contains("is not a provisional Wall")
        );
    }

    #[test]
    fn construction_validation_requires_completed_wall_to_be_permanent() {
        let mut valid_world = candidate_world();
        let site = valid_world
            .spawn({
                let mut site = WallConstructionSite::new(
                    hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                    Vec2::ZERO,
                    1,
                );
                site.phase = WallConstructionPhase::Coating;
                site
            })
            .id();
        let permanent_wall = valid_world
            .spawn(Building {
                kind: BuildingType::Wall,
                is_provisional: false,
            })
            .id();
        let mut tile = WallTileBlueprint::new(site, (7, 7));
        tile.state = WallTileState::Complete;
        tile.spawned_wall = Some(permanent_wall);
        valid_world.spawn(tile.clone());
        valid_world
            .resource_mut::<WorldMap>()
            .set_building((7, 7), permanent_wall);
        validate_construction_links(&valid_world).unwrap();

        let mut invalid_world = candidate_world();
        let site = invalid_world
            .spawn({
                let mut site = WallConstructionSite::new(
                    hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                    Vec2::ZERO,
                    1,
                );
                site.phase = WallConstructionPhase::Coating;
                site
            })
            .id();
        let provisional_wall = invalid_world
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
            ))
            .id();
        tile.parent_site = site;
        tile.spawned_wall = Some(provisional_wall);
        invalid_world.spawn(tile);
        invalid_world
            .resource_mut::<WorldMap>()
            .set_building((7, 7), provisional_wall);

        assert!(
            validate_construction_links(&invalid_world)
                .unwrap_err()
                .contains("is still provisional")
        );
    }

    #[test]
    fn durable_topology_validation_rejects_missing_or_out_of_bounds_natural_obstacles() {
        let mut missing_position = candidate_world();
        missing_position.spawn((Tree, TreeVariant(0), Transform::default()));
        assert!(
            validate_durable_topology_candidate(&missing_position)
                .unwrap_err()
                .contains("has no ObstaclePosition")
        );

        let mut out_of_bounds = candidate_world();
        out_of_bounds.spawn((
            Rock,
            ObstaclePosition(MAP_WIDTH, MAP_HEIGHT - 1),
            Transform::default(),
        ));
        assert!(
            validate_durable_topology_candidate(&out_of_bounds)
                .unwrap_err()
                .contains("out-of-bounds ObstaclePosition")
        );
    }

    #[test]
    fn durable_topology_validation_rejects_orphan_construction_tile() {
        let mut world = candidate_world();
        let missing_site = world.spawn_empty().id();
        world.despawn(missing_site);
        world.spawn(FloorTileBlueprint::new(missing_site, (4, 5)));

        assert!(
            validate_durable_topology_candidate(&world)
                .unwrap_err()
                .contains("references missing parent site")
        );
    }

    #[test]
    fn durable_topology_validation_rejects_mistyped_energy_and_zone_links() {
        let mut energy_world = candidate_world();
        let wrong_grid = energy_world.spawn_empty().id();
        energy_world.spawn((PowerGenerator::default(), GeneratesFor(wrong_grid)));
        energy_world.flush();
        assert!(
            validate_durable_topology_candidate(&energy_world)
                .unwrap_err()
                .contains("is not a PowerGrid")
        );

        let mut zone_world = candidate_world();
        let yard = zone_world
            .spawn(Yard {
                min: Vec2::ZERO,
                max: Vec2::ONE,
            })
            .id();
        zone_world.spawn((
            Site {
                min: Vec2::ZERO,
                max: Vec2::ONE,
            },
            PairedYard(yard),
        ));
        assert!(
            validate_durable_topology_candidate(&zone_world)
                .unwrap_err()
                .contains("are not symmetric")
        );
    }

    #[test]
    fn energy_validation_rejects_soul_spa_map_footprint_mismatch() {
        let mut world = candidate_world();
        let site = world
            .spawn((
                SoulSpaSite::default(),
                Building {
                    kind: BuildingType::SoulSpa,
                    is_provisional: false,
                },
                PowerGenerator::default(),
                Transform::default(),
            ))
            .id();
        for grid in [(8, 8), (9, 8), (8, 9), (9, 9)] {
            world.spawn((
                SoulSpaTile {
                    parent_site: site,
                    grid_pos: grid,
                },
                Transform::default(),
            ));
        }
        for grid in [(8, 8), (9, 8), (8, 9)] {
            world.resource_mut::<WorldMap>().set_building(grid, site);
        }

        assert!(
            validate_energy_links(&world)
                .unwrap_err()
                .contains("is not owned by site")
        );
    }

    #[test]
    fn task_logistics_validation_accepts_consistent_durable_owners() {
        let mut world = candidate_world();
        let familiar = world.spawn(Familiar::default()).id();
        let task = world
            .spawn((
                hw_jobs::Designation {
                    work_type: hw_core::jobs::WorkType::Haul,
                },
                ManagedBy(familiar),
            ))
            .id();
        let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
        let wheelbarrow = world
            .spawn((
                ResourceItem(hw_core::logistics::ResourceType::Wheelbarrow),
                Wheelbarrow { capacity: 2 },
                BelongsTo(parking),
                ParkedAt(parking),
                Transform::default(),
            ))
            .id();
        world.spawn((
            ResourceItem(hw_core::logistics::ResourceType::Sand),
            LoadedIn(wheelbarrow),
            Transform::default(),
        ));
        world.flush();

        validate_task_logistics_candidate(&world).unwrap();
        assert!(world.get::<ManagedTasks>(familiar).unwrap().contains(task));
        assert!(world.get::<LoadedItems>(wheelbarrow).is_some());
        assert!(world.get::<ParkedWheelbarrows>(parking).is_some());
    }

    #[test]
    fn task_logistics_validation_rejects_missing_inventory_entity() {
        let mut world = candidate_world();
        let missing = world.spawn_empty().id();
        world.despawn(missing);
        world.spawn((
            DamnedSoul::default(),
            Inventory(Some(missing)),
            Transform::default(),
        ));

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("missing entity")
        );
    }

    #[test]
    fn task_logistics_validation_rejects_over_capacity_loaded_items() {
        let mut world = candidate_world();
        let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
        let wheelbarrow = world
            .spawn((
                ResourceItem(hw_core::logistics::ResourceType::Wheelbarrow),
                Wheelbarrow { capacity: 1 },
                BelongsTo(parking),
                Transform::default(),
            ))
            .id();
        for _ in 0..2 {
            world.spawn((
                ResourceItem(hw_core::logistics::ResourceType::Wood),
                LoadedIn(wheelbarrow),
                Transform::default(),
            ));
        }
        world.flush();

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("exceeding capacity")
        );
    }

    #[test]
    fn task_logistics_validation_rejects_non_loadable_wheelbarrow_cargo() {
        let mut world = candidate_world();
        let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
        let wheelbarrow = world
            .spawn((
                ResourceItem(ResourceType::Wheelbarrow),
                Wheelbarrow { capacity: 1 },
                BelongsTo(parking),
                Transform::default(),
            ))
            .id();
        world.spawn((
            ResourceItem(ResourceType::BucketEmpty),
            LoadedIn(wheelbarrow),
            Transform::default(),
        ));
        world.flush();

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("non-loadable resource type")
        );
    }

    #[test]
    fn task_logistics_validation_rejects_wheelbarrow_in_a_durable_container() {
        let mut world = candidate_world();
        let parking = world.spawn(WheelbarrowParking { capacity: 1 }).id();
        let stockpile = world
            .spawn(Stockpile {
                capacity: 1,
                resource_type: None,
            })
            .id();
        world.spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 1 },
            BelongsTo(parking),
            StoredIn(stockpile),
            Transform::default(),
        ));
        world.flush();

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("cannot have a durable container owner")
        );
    }

    #[test]
    fn task_logistics_validation_rejects_missing_belongs_to_owner() {
        let mut world = candidate_world();
        let missing = world.spawn_empty().id();
        world.despawn(missing);
        world.spawn((
            ResourceItem(ResourceType::Wood),
            BelongsTo(missing),
            Transform::default(),
        ));

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("references missing owner")
        );
    }

    #[test]
    fn task_logistics_validation_rejects_stockpile_content_type_mismatch() {
        let mut world = candidate_world();
        let stockpile = world
            .spawn(Stockpile {
                capacity: 2,
                resource_type: Some(ResourceType::Wood),
            })
            .id();
        world.spawn((
            ResourceItem(ResourceType::Rock),
            StoredIn(stockpile),
            Transform::default(),
        ));
        world.flush();

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("resource type does not match")
        );
    }

    #[test]
    fn task_logistics_validation_recognizes_bucket_storage_from_durable_tank_owner() {
        let mut world = candidate_world();
        let tank = world
            .spawn(Building {
                kind: BuildingType::Tank,
                ..default()
            })
            .id();
        let storage = world
            .spawn((
                Stockpile {
                    capacity: 2,
                    resource_type: None,
                },
                BelongsTo(tank),
            ))
            .id();
        world.spawn((
            ResourceItem(ResourceType::BucketEmpty),
            BelongsTo(tank),
            StoredIn(storage),
            Transform::default(),
        ));
        world.flush();

        validate_task_logistics_candidate(&world).unwrap();
        assert!(world.get::<BucketStorage>(storage).is_none());
    }

    #[test]
    fn task_logistics_validation_rejects_noncanonical_manual_request_slots() {
        let mut world = candidate_world();
        let familiar = world.spawn(Familiar::default()).id();
        let stockpile = world
            .spawn(Stockpile {
                capacity: 2,
                resource_type: None,
            })
            .id();
        let source = world
            .spawn((
                ResourceItem(ResourceType::Wood),
                ManualHaulPinnedSource,
                Transform::default(),
            ))
            .id();
        world.spawn((
            TransportRequest {
                kind: TransportRequestKind::DepositToStockpile,
                anchor: stockpile,
                resource_type: ResourceType::Wood,
                issued_by: familiar,
                priority: hw_logistics::transport_request::TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TransportRequestFixedSource(source),
            ManualTransportRequest,
            TransportDemand {
                desired_slots: 2,
                inflight: 0,
            },
            TransportPolicy::default(),
            ManagedBy(familiar),
            Designation {
                work_type: WorkType::Haul,
            },
            TaskSlots::new(2),
            Priority(0),
            Transform::default(),
        ));
        world.flush();

        assert!(
            validate_task_logistics_candidate(&world)
                .unwrap_err()
                .contains("non-canonical active shape")
        );
    }
}
