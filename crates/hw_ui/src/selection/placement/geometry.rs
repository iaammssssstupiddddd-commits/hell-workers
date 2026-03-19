use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use hw_jobs::BuildingType;

use super::PlacementGeometry;

fn grid_to_world(x: i32, y: i32) -> Vec2 {
    Vec2::new(
        (x as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * TILE_SIZE,
        (y as f32 - (MAP_HEIGHT as f32 - 1.0) / 2.0) * TILE_SIZE,
    )
}

fn world_to_grid(pos: Vec2) -> (i32, i32) {
    let x = (pos.x / TILE_SIZE + (MAP_WIDTH as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
    let y = (pos.y / TILE_SIZE + (MAP_HEIGHT as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
    (x, y)
}

/// Returns the anchor grid for a building move operation.
/// For 2×2 buildings the cursor is treated as the building center, so the anchor is
/// shifted by half a tile to the bottom-left.
pub fn move_anchor_grid(kind: BuildingType, world_pos: Vec2) -> (i32, i32) {
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => {
            world_to_grid(world_pos - Vec2::splat(TILE_SIZE * 0.5))
        }
        _ => world_to_grid(world_pos),
    }
}

/// Returns the occupied grid tiles for a moved building given its anchor.
/// Equivalent to `building_occupied_grids` but without the bridge special-case.
pub fn move_occupied_grids(kind: BuildingType, anchor: (i32, i32)) -> Vec<(i32, i32)> {
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => vec![
            anchor,
            (anchor.0 + 1, anchor.1),
            (anchor.0, anchor.1 + 1),
            (anchor.0 + 1, anchor.1 + 1),
        ],
        _ => vec![anchor],
    }
}

/// Returns the draw/spawn position for a moved building given its anchor grid.
pub fn move_spawn_pos(kind: BuildingType, anchor: (i32, i32)) -> Vec2 {
    let base = grid_to_world(anchor.0, anchor.1);
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => base + Vec2::splat(TILE_SIZE * 0.5),
        _ => base,
    }
}

pub fn building_geometry(
    building_type: BuildingType,
    grid: (i32, i32),
    river_y_min: i32,
) -> PlacementGeometry {
    let occupied_grids = building_occupied_grids(building_type, grid, river_y_min);
    let draw_pos = building_spawn_pos(building_type, grid, river_y_min);
    let size = building_size(building_type);
    PlacementGeometry {
        occupied_grids,
        draw_pos,
        size,
    }
}

pub fn bucket_storage_geometry(anchor_grid: (i32, i32)) -> PlacementGeometry {
    PlacementGeometry {
        occupied_grids: vec![anchor_grid, (anchor_grid.0 + 1, anchor_grid.1)],
        draw_pos: grid_to_world(anchor_grid.0, anchor_grid.1) + Vec2::new(TILE_SIZE * 0.5, 0.0),
        size: Vec2::new(TILE_SIZE * 2.0, TILE_SIZE),
    }
}

pub fn building_occupied_grids(
    building_type: BuildingType,
    grid: (i32, i32),
    river_y_min: i32,
) -> Vec<(i32, i32)> {
    match building_type {
        BuildingType::Bridge => (0..5)
            .flat_map(|dy| [(grid.0, river_y_min + dy), (grid.0 + 1, river_y_min + dy)])
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

pub fn building_spawn_pos(building_type: BuildingType, grid: (i32, i32), river_y_min: i32) -> Vec2 {
    let base_pos = grid_to_world(grid.0, grid.1);
    match building_type {
        BuildingType::Bridge => {
            let base = grid_to_world(grid.0, river_y_min);
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

pub fn building_size(building_type: BuildingType) -> Vec2 {
    match building_type {
        BuildingType::Bridge => Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => Vec2::splat(TILE_SIZE * 2.0),
        _ => Vec2::splat(TILE_SIZE),
    }
}

pub fn grid_is_nearby(base: (i32, i32), target: (i32, i32), tiles: i32) -> bool {
    (target.0 - base.0).abs() <= tiles && (target.1 - base.1).abs() <= tiles
}
