use crate::systems::jobs::BuildingType;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_ui::selection::{
    PlacementValidation, TANK_NEARBY_BUCKET_STORAGE_TILES, WorldReadApi, bucket_storage_geometry,
    move_occupied_grids, validate_moved_bucket_storage_placement,
};

struct MoveCompanionWorld<'a>(&'a WorldMap);

impl WorldReadApi for MoveCompanionWorld<'_> {
    fn has_building(&self, grid: (i32, i32)) -> bool {
        self.0.has_building(grid)
    }

    fn has_stockpile(&self, grid: (i32, i32)) -> bool {
        self.0.has_stockpile(grid)
    }

    fn is_walkable(&self, gx: i32, gy: i32) -> bool {
        self.0.is_walkable(gx, gy)
    }

    fn is_river_tile(&self, gx: i32, gy: i32) -> bool {
        self.0.is_river_tile(gx, gy)
    }

    fn building_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.0.building_entity(grid)
    }

    fn stockpile_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.0.stockpile_entity(grid)
    }

    fn pos_to_idx(&self, gx: i32, gy: i32) -> Option<usize> {
        self.0.pos_to_idx(gx, gy)
    }
}

pub(crate) fn validate_tank_companion_for_move(
    world_map: &WorldMap,
    building_entity: Entity,
    parent_anchor: (i32, i32),
    companion_anchor: (i32, i32),
    old_building_occupied: &[(i32, i32)],
    q_bucket_storages: &Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
) -> PlacementValidation {
    let geometry = bucket_storage_geometry(companion_anchor);
    let parent_occupied = move_occupied_grids(BuildingType::Tank, parent_anchor);
    let own_companion_grids =
        own_bucket_storage_grids(world_map, building_entity, q_bucket_storages);

    validate_moved_bucket_storage_placement(
        &MoveCompanionWorld(world_map),
        &geometry,
        &parent_occupied,
        old_building_occupied,
        &own_companion_grids,
        TANK_NEARBY_BUCKET_STORAGE_TILES,
    )
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
        .stockpile_entries()
        .filter_map(|(grid, stockpile_entity)| {
            q_bucket_storages
                .get(*stockpile_entity)
                .ok()
                .filter(|(_, belongs_to)| belongs_to.0 == building_entity)
                .map(|_| *grid)
        })
        .collect()
}
