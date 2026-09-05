use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_jobs::{BuildingAnchorBasis, BuildingType, building_shape};
use hw_ui::selection::PlacementGeometry;
use hw_world::WorldMap;

pub(crate) fn building_geometry(
    kind: BuildingType,
    clicked_grid: (i32, i32),
    river_y_min: i32,
) -> PlacementGeometry {
    PlacementGeometry {
        occupied_grids: building_occupied_grids(kind, clicked_grid, river_y_min),
        draw_pos: building_spawn_pos(kind, clicked_grid, river_y_min),
        size: building_size(kind),
    }
}

pub(crate) fn building_occupied_grids(
    kind: BuildingType,
    clicked_grid: (i32, i32),
    river_y_min: i32,
) -> Vec<(i32, i32)> {
    let shape = building_shape(kind);
    let anchor = match shape.anchor_basis {
        BuildingAnchorBasis::ClickedGrid => clicked_grid,
        BuildingAnchorBasis::RiverYMin => (clicked_grid.0, river_y_min),
    };
    shape
        .ordered_relative_tiles
        .iter()
        .map(|offset| (anchor.0 + offset.0, anchor.1 + offset.1))
        .collect()
}

pub(crate) fn building_spawn_pos(
    kind: BuildingType,
    clicked_grid: (i32, i32),
    river_y_min: i32,
) -> Vec2 {
    let shape = building_shape(kind);
    let anchor = match shape.anchor_basis {
        BuildingAnchorBasis::ClickedGrid => clicked_grid,
        BuildingAnchorBasis::RiverYMin => (clicked_grid.0, river_y_min),
    };
    WorldMap::grid_to_world(anchor.0, anchor.1)
        + Vec2::new(
            shape.center_offset_tiles.0 * TILE_SIZE,
            shape.center_offset_tiles.1 * TILE_SIZE,
        )
}

pub(crate) fn building_size(kind: BuildingType) -> Vec2 {
    let shape = building_shape(kind);
    Vec2::new(
        shape.size_tiles.0 * TILE_SIZE,
        shape.size_tiles.1 * TILE_SIZE,
    )
}

pub(crate) fn bucket_storage_geometry(anchor_grid: (i32, i32)) -> PlacementGeometry {
    let anchor_world = WorldMap::grid_to_world(anchor_grid.0, anchor_grid.1);
    let mut geometry = hw_ui::selection::bucket_storage_geometry(anchor_world);
    geometry.occupied_grids = vec![anchor_grid, (anchor_grid.0 + 1, anchor_grid.1)];
    geometry
}

/// Restores the old anchor from a committed transform. Unsupported or stale
/// move contexts retain the former single-tile fallback.
pub(crate) fn existing_movable_building_anchor(
    kind: BuildingType,
    world_position: Vec2,
) -> (i32, i32) {
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => {
            WorldMap::world_to_grid(world_position - Vec2::splat(TILE_SIZE * 0.5))
        }
        _ => WorldMap::world_to_grid(world_position),
    }
}

pub(crate) fn move_occupied_grids(kind: BuildingType, anchor: (i32, i32)) -> Vec<(i32, i32)> {
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

pub(crate) fn move_spawn_pos(kind: BuildingType, anchor: (i32, i32)) -> Vec2 {
    let base = WorldMap::grid_to_world(anchor.0, anchor.1);
    match kind {
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => base + Vec2::splat(TILE_SIZE * 0.5),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_uses_river_anchor_and_preserves_order() {
        let geometry = building_geometry(BuildingType::Bridge, (4, 99), 7);
        assert_eq!(geometry.occupied_grids[0], (4, 7));
        assert_eq!(geometry.occupied_grids[1], (5, 7));
        assert_eq!(geometry.occupied_grids[9], (5, 11));
        assert_eq!(
            geometry.draw_pos,
            WorldMap::grid_to_world(4, 7) + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 2.0)
        );
    }

    #[test]
    fn soul_spa_projection_keeps_its_southward_draw_offset() {
        let geometry = building_geometry(BuildingType::SoulSpa, (10, 10), 0);
        assert_eq!(
            geometry.occupied_grids,
            vec![(10, 10), (11, 10), (10, 9), (11, 9)]
        );
        assert_eq!(
            geometry.draw_pos,
            WorldMap::grid_to_world(10, 10) + Vec2::new(TILE_SIZE * 0.5, -TILE_SIZE * 0.5)
        );
    }

    #[test]
    fn old_transform_and_new_cursor_use_distinct_anchor_rules() {
        let anchor = (12, 8);
        let center = move_spawn_pos(BuildingType::Tank, anchor);
        assert_eq!(
            existing_movable_building_anchor(BuildingType::Tank, center),
            anchor
        );
        assert_eq!(WorldMap::world_to_grid(center), (13, 9));
        assert_eq!(
            existing_movable_building_anchor(BuildingType::Bridge, center),
            WorldMap::world_to_grid(center)
        );
    }
}
