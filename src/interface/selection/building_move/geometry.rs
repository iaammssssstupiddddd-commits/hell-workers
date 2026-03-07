use hw_core::constants::TILE_SIZE;
use crate::systems::jobs::BuildingType;
use crate::world::map::WorldMap;
use bevy::prelude::*;

fn is_two_by_two(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Tank
            | BuildingType::MudMixer
            | BuildingType::RestArea
            | BuildingType::WheelbarrowParking
    )
}

pub(super) fn anchor_grid_for_kind(kind: BuildingType, world_pos: Vec2) -> (i32, i32) {
    if is_two_by_two(kind) {
        WorldMap::world_to_grid(world_pos - Vec2::splat(TILE_SIZE * 0.5))
    } else {
        WorldMap::world_to_grid(world_pos)
    }
}

pub(super) fn spawn_pos_for_kind(kind: BuildingType, anchor_grid: (i32, i32)) -> Vec2 {
    let base = WorldMap::grid_to_world(anchor_grid.0, anchor_grid.1);
    if is_two_by_two(kind) {
        base + Vec2::splat(TILE_SIZE * 0.5)
    } else {
        base
    }
}

pub(super) fn occupied_grids_for_kind(
    kind: BuildingType,
    anchor_grid: (i32, i32),
) -> Vec<(i32, i32)> {
    if is_two_by_two(kind) {
        vec![
            anchor_grid,
            (anchor_grid.0 + 1, anchor_grid.1),
            (anchor_grid.0, anchor_grid.1 + 1),
            (anchor_grid.0 + 1, anchor_grid.1 + 1),
        ]
    } else {
        vec![anchor_grid]
    }
}
