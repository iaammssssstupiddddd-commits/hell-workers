use crate::constants::TILE_SIZE;
use crate::systems::jobs::BuildingType;
use crate::world::map::{RIVER_Y_MIN, WorldMap};
use bevy::prelude::Vec2;

pub(super) fn occupied_grids_for_building(
    building_type: BuildingType,
    grid: (i32, i32),
) -> Vec<(i32, i32)> {
    match building_type {
        BuildingType::Bridge => (0..5)
            .flat_map(|dy| [(grid.0, RIVER_Y_MIN + dy), (grid.0 + 1, RIVER_Y_MIN + dy)])
            .collect(),
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => vec![
            grid,
            (grid.0 + 1, grid.1),
            (grid.0, grid.1 + 1),
            (grid.0 + 1, grid.1 + 1),
        ],
        _ => vec![grid],
    }
}

pub(super) fn building_spawn_pos(building_type: BuildingType, grid: (i32, i32)) -> Vec2 {
    let base_pos = WorldMap::grid_to_world(grid.0, grid.1);
    match building_type {
        BuildingType::Bridge => {
            let base = WorldMap::grid_to_world(grid.0, RIVER_Y_MIN);
            base + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 2.0)
        }
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => {
            base_pos + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5)
        }
        _ => base_pos,
    }
}

pub(super) fn building_size(building_type: BuildingType) -> Vec2 {
    match building_type {
        BuildingType::Bridge => Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => Vec2::splat(TILE_SIZE * 2.0),
        _ => Vec2::splat(TILE_SIZE),
    }
}

pub(super) fn grid_is_nearby(base: (i32, i32), target: (i32, i32), tiles: i32) -> bool {
    (target.0 - base.0).abs() <= tiles && (target.1 - base.1).abs() <= tiles
}
