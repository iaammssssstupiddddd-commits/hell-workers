use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::door_rules::is_valid_door_placement;
use super::geometry::{building_size, building_spawn_pos, occupied_grids_for_building};

const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;

/// Attempts to spawn a Blueprint entity for the given building type at the given grid position.
/// Returns `Some((entity, occupied_grids, spawn_pos))` on success, `None` if placement is blocked.
pub(super) fn place_building_blueprint(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    building_type: BuildingType,
    grid: (i32, i32),
    q_buildings: &Query<&Building>,
    q_blueprints_by_entity: &Query<&Blueprint>,
) -> Option<(Entity, Vec<(i32, i32)>, Vec2)> {
    let occupied_grids = occupied_grids_for_building(building_type, grid);
    let spawn_pos = building_spawn_pos(building_type, grid);
    let custom_size = Some(building_size(building_type));

    let replace_wall_entity = if building_type == BuildingType::Door {
        world_map.buildings.get(&grid).copied().filter(|entity| {
            q_buildings.get(*entity).is_ok_and(|building| {
                building.kind == BuildingType::Wall && !building.is_provisional
            })
        })
    } else {
        None
    };

    let can_place = if building_type == BuildingType::Bridge {
        occupied_grids.iter().all(|&g| {
            !world_map.buildings.contains_key(&g)
                && !world_map.stockpiles.contains_key(&g)
                && world_map.is_river_tile(g.0, g.1)
        })
    } else if building_type == BuildingType::Door {
        let base_tile_ok = if replace_wall_entity.is_some() {
            !world_map.stockpiles.contains_key(&grid)
        } else {
            !world_map.buildings.contains_key(&grid)
                && !world_map.stockpiles.contains_key(&grid)
                && world_map.is_walkable(grid.0, grid.1)
        };
        base_tile_ok
            && is_valid_door_placement(world_map, q_buildings, q_blueprints_by_entity, grid)
    } else {
        occupied_grids.iter().all(|&g| {
            !world_map.buildings.contains_key(&g)
                && !world_map.stockpiles.contains_key(&g)
                && world_map.is_walkable(g.0, g.1)
        })
    };
    if !can_place {
        return None;
    }

    if let Some(entity) = replace_wall_entity {
        world_map.buildings.remove(&grid);
        world_map.remove_obstacle(grid.0, grid.1);
        commands.entity(entity).despawn();
    }

    let texture = match building_type {
        BuildingType::Wall => game_assets.wall_isolated.clone(),
        BuildingType::Door => game_assets.door_closed.clone(),
        BuildingType::Floor => {
            unreachable!("Floor should be placed via Drag-and-drop area selection")
        }
        BuildingType::Tank => game_assets.tank_empty.clone(),
        BuildingType::MudMixer => game_assets.mud_mixer.clone(),
        BuildingType::RestArea => game_assets.rest_area.clone(),
        BuildingType::Bridge => game_assets.bridge.clone(),
        BuildingType::SandPile => game_assets.sand_pile.clone(),
        BuildingType::BonePile => game_assets.bone_pile.clone(),
        BuildingType::WheelbarrowParking => game_assets.wheelbarrow_parking.clone(),
    };

    let entity = commands
        .spawn((
            Blueprint::new(building_type, occupied_grids.clone()),
            crate::systems::jobs::Designation {
                work_type: crate::systems::jobs::WorkType::Build,
            },
            crate::systems::jobs::TaskSlots::new(1),
            Sprite {
                image: texture,
                color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                custom_size,
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_AURA),
            Name::new(format!("Blueprint ({:?})", building_type)),
        ))
        .id();

    for &g in &occupied_grids {
        world_map.buildings.insert(g, entity);
        if building_type != BuildingType::Bridge {
            world_map.add_obstacle(g.0, g.1);
        }
    }

    Some((entity, occupied_grids, spawn_pos))
}

/// Attempts to place the BucketStorage companion for a Tank blueprint.
/// Returns true if placement succeeded.
pub(super) fn try_place_bucket_storage_companion(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    parent_blueprint: Entity,
    parent_occupied_grids: &[(i32, i32)],
    anchor_grid: (i32, i32),
) -> bool {
    use super::geometry::grid_is_nearby;

    let storage_grids = [anchor_grid, (anchor_grid.0 + 1, anchor_grid.1)];
    let is_near_parent = storage_grids.iter().all(|&storage_grid| {
        parent_occupied_grids.iter().any(|&parent_grid| {
            grid_is_nearby(parent_grid, storage_grid, TANK_NEARBY_BUCKET_STORAGE_TILES)
        })
    });
    if !is_near_parent {
        return false;
    }

    let can_place = storage_grids.iter().all(|&(gx, gy)| {
        !world_map.buildings.contains_key(&(gx, gy))
            && !world_map.stockpiles.contains_key(&(gx, gy))
            && world_map.is_walkable(gx, gy)
    });
    if !can_place {
        return false;
    }

    for (gx, gy) in storage_grids {
        let pos = WorldMap::grid_to_world(gx, gy);
        let storage_entity = commands
            .spawn((
                crate::systems::logistics::Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                crate::systems::logistics::BucketStorage,
                crate::systems::logistics::PendingBelongsToBlueprint(parent_blueprint),
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                Name::new("Pending Tank Bucket Storage"),
            ))
            .id();
        world_map.stockpiles.insert((gx, gy), storage_entity);
    }
    true
}
