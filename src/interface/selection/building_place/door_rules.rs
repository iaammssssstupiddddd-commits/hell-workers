use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub(super) fn is_valid_door_placement(
    world_map: &WorldMap,
    q_buildings: &Query<&Building>,
    q_blueprints_by_entity: &Query<&Blueprint>,
    grid: (i32, i32),
) -> bool {
    let left = is_wall_or_door_at(
        world_map,
        q_buildings,
        q_blueprints_by_entity,
        (grid.0 - 1, grid.1),
    );
    let right = is_wall_or_door_at(
        world_map,
        q_buildings,
        q_blueprints_by_entity,
        (grid.0 + 1, grid.1),
    );
    let up = is_wall_or_door_at(
        world_map,
        q_buildings,
        q_blueprints_by_entity,
        (grid.0, grid.1 + 1),
    );
    let down = is_wall_or_door_at(
        world_map,
        q_buildings,
        q_blueprints_by_entity,
        (grid.0, grid.1 - 1),
    );
    (left && right) || (up && down)
}

fn is_wall_or_door_at(
    world_map: &WorldMap,
    q_buildings: &Query<&Building>,
    q_blueprints_by_entity: &Query<&Blueprint>,
    grid: (i32, i32),
) -> bool {
    let Some(&entity) = world_map.buildings.get(&grid) else {
        return false;
    };
    if let Ok(building) = q_buildings.get(entity) {
        return matches!(building.kind, BuildingType::Wall | BuildingType::Door);
    }
    if let Ok(blueprint) = q_blueprints_by_entity.get(entity) {
        return matches!(blueprint.kind, BuildingType::Wall | BuildingType::Door);
    }
    false
}
