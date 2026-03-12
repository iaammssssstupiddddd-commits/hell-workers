use crate::systems::jobs::BuildingType;
use crate::world::map::{WorldMap, WorldMapRef};
use bevy::prelude::*;
use hw_ui::selection::{
    PlacementValidation, TANK_NEARBY_BUCKET_STORAGE_TILES, bucket_storage_geometry,
    move_occupied_grids, validate_moved_bucket_storage_placement,
};

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
        &WorldMapRef(world_map),
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
