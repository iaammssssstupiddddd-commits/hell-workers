use crate::systems::jobs::BuildingType;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::geometry::occupied_grids_for_kind;

const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;

pub(super) fn can_place_tank_companion_for_move(
    world_map: &WorldMap,
    building_entity: Entity,
    parent_anchor: (i32, i32),
    companion_anchor: (i32, i32),
    old_building_occupied: &[(i32, i32)],
    q_bucket_storages: &Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
) -> bool {
    let companion_grids = [companion_anchor, (companion_anchor.0 + 1, companion_anchor.1)];
    let parent_occupied = occupied_grids_for_kind(BuildingType::Tank, parent_anchor);
    let own_companion_grids = own_bucket_storage_grids(world_map, building_entity, q_bucket_storages);

    let near_parent = companion_grids.iter().all(|&storage_grid| {
        parent_occupied.iter().any(|&parent_grid| {
            (storage_grid.0 - parent_grid.0).abs() <= TANK_NEARBY_BUCKET_STORAGE_TILES
                && (storage_grid.1 - parent_grid.1).abs() <= TANK_NEARBY_BUCKET_STORAGE_TILES
        })
    });
    if !near_parent {
        return false;
    }

    companion_grids.iter().all(|&(gx, gy)| {
        world_map.pos_to_idx(gx, gy).is_some()
            && (!world_map.buildings.contains_key(&(gx, gy))
                || old_building_occupied.contains(&(gx, gy)))
            && (world_map.stockpiles.get(&(gx, gy)).is_none()
                || own_companion_grids.contains(&(gx, gy)))
            && (world_map.is_walkable(gx, gy)
                || old_building_occupied.contains(&(gx, gy))
                || own_companion_grids.contains(&(gx, gy)))
    })
}

pub(super) fn can_place_moved_building(
    world_map: &WorldMap,
    building_entity: Entity,
    old_occupied: &[(i32, i32)],
    destination_occupied: &[(i32, i32)],
) -> bool {
    destination_occupied.iter().all(|&(gx, gy)| {
        let Some(_) = world_map.pos_to_idx(gx, gy) else {
            return false;
        };

        let occupied_by_other_building = world_map
            .buildings
            .get(&(gx, gy))
            .is_some_and(|entity| *entity != building_entity);
        if occupied_by_other_building {
            return false;
        }

        if world_map.stockpiles.contains_key(&(gx, gy)) {
            return false;
        }

        world_map.is_walkable(gx, gy) || old_occupied.contains(&(gx, gy))
    })
}

fn own_bucket_storage_grids(
    world_map: &WorldMap,
    building_entity: Entity,
    q_bucket_storages: &Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
) -> Vec<(i32, i32)> {
    world_map
        .stockpiles
        .iter()
        .filter_map(|(grid, stockpile_entity)| {
            q_bucket_storages
                .get(*stockpile_entity)
                .ok()
                .filter(|(_, belongs_to)| belongs_to.0 == building_entity)
                .map(|_| *grid)
        })
        .collect()
}
