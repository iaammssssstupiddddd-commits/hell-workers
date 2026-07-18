use crate::world::map::{WorldMap, WorldMapRef};
use hw_ui::selection::{
    AreaPlacementPlan, PlacementRejectReason, build_area_placement_plan, validate_area_size,
    validate_floor_tile as shared_validate_floor_tile, validate_wall_area,
    validate_wall_tile as shared_validate_wall_tile,
};
use std::collections::HashSet;

/// Validate a single tile for floor placement. Returns `None` if valid, or a reject reason.
pub(crate) fn validate_floor_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason> {
    shared_validate_floor_tile(
        &WorldMapRef(world_map),
        (gx, gy),
        existing_floor_tile_grids,
        existing_floor_building_grids,
    )
}

/// Validate a single tile for wall placement. Returns `None` if valid, or a reject reason.
pub(crate) fn validate_wall_tile(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> Option<PlacementRejectReason> {
    shared_validate_wall_tile(
        &WorldMapRef(world_map),
        (gx, gy),
        existing_floor_building_grids,
    )
}

/// 床の有無チェックを省いた壁タイルバリデーション（デバッグ用）。
/// 占有・歩行可能チェックのみ行う。
pub(crate) fn validate_wall_tile_no_floor_check(
    gx: i32,
    gy: i32,
    world_map: &WorldMap,
) -> Option<PlacementRejectReason> {
    // 対象グリッド自体を "floor あり" として渡すことで floor チェックのみ通過させる
    let fake_floor = HashSet::from([(gx, gy)]);
    shared_validate_wall_tile(&WorldMapRef(world_map), (gx, gy), &fake_floor)
}

pub(crate) fn build_floor_placement_plan(
    area: &crate::systems::command::TaskArea,
    world_map: &WorldMap,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
) -> AreaPlacementPlan {
    let (min_grid, max_grid) = area_grid_bounds(area);
    let width = max_grid.0 - min_grid.0 + 1;
    let height = max_grid.1 - min_grid.1 + 1;
    build_area_placement_plan(
        min_grid,
        max_grid,
        validate_area_size(width, height),
        |(gx, gy)| {
            validate_floor_tile(
                gx,
                gy,
                world_map,
                existing_floor_tile_grids,
                existing_floor_building_grids,
            )
        },
    )
}

pub(crate) fn build_wall_placement_plan(
    area: &crate::systems::command::TaskArea,
    world_map: &WorldMap,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
    bypass_floor_check: bool,
) -> AreaPlacementPlan {
    let (min_grid, max_grid) = area_grid_bounds(area);
    let width = max_grid.0 - min_grid.0 + 1;
    let height = max_grid.1 - min_grid.1 + 1;
    build_area_placement_plan(
        min_grid,
        max_grid,
        validate_wall_area(width, height),
        |(gx, gy)| {
            if bypass_floor_check {
                validate_wall_tile_no_floor_check(gx, gy, world_map)
            } else {
                validate_wall_tile(gx, gy, world_map, existing_floor_building_grids)
            }
        },
    )
}

pub(crate) fn existing_floor_building_grids(
    q_floor_buildings: &bevy::prelude::Query<(
        &crate::systems::jobs::Building,
        &bevy::prelude::Transform,
    )>,
) -> HashSet<(i32, i32)> {
    use crate::systems::jobs::BuildingType;
    q_floor_buildings
        .iter()
        .filter(|&(building, _)| building.kind == BuildingType::Floor)
        .map(|(_, transform)| WorldMap::world_to_grid(transform.translation.truncate()))
        .collect()
}

fn area_grid_bounds(area: &crate::systems::command::TaskArea) -> ((i32, i32), (i32, i32)) {
    (
        WorldMap::world_to_grid(area.min() + bevy::prelude::Vec2::splat(0.1)),
        WorldMap::world_to_grid(area.max() - bevy::prelude::Vec2::splat(0.1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::TILE_SIZE;
    use hw_ui::selection::PlacementFeedbackStatus;

    fn area_for_grids(min: (i32, i32), max: (i32, i32)) -> crate::systems::command::TaskArea {
        let half = bevy::prelude::Vec2::splat(TILE_SIZE * 0.5);
        crate::systems::command::TaskArea::from_points(
            WorldMap::grid_to_world(min.0, min.1) - half,
            WorldMap::grid_to_world(max.0, max.1) + half,
        )
    }

    #[test]
    fn floor_placement_plan_preserves_partial_success_and_zero_valid_rejection() {
        let world_map = WorldMap::default();
        let area = area_for_grids((0, 0), (1, 0));
        let partial = build_floor_placement_plan(
            &area,
            &world_map,
            &HashSet::from([(1, 0)]),
            &HashSet::new(),
        );
        assert_eq!(partial.valid_tiles, vec![(0, 0)]);
        assert_eq!(
            partial.feedback().unwrap().status,
            PlacementFeedbackStatus::Partial
        );

        let rejected = build_floor_placement_plan(
            &area,
            &world_map,
            &HashSet::from([(0, 0), (1, 0)]),
            &HashSet::new(),
        );
        assert!(rejected.valid_tiles.is_empty());
        assert_eq!(
            rejected.feedback().unwrap().status,
            PlacementFeedbackStatus::Rejected
        );
    }

    #[test]
    fn wall_placement_plan_uses_same_floor_requirement_for_preview_and_commit() {
        let world_map = WorldMap::default();
        let area = area_for_grids((0, 0), (1, 0));
        let no_floors = build_wall_placement_plan(&area, &world_map, &HashSet::new(), false);
        assert!(no_floors.valid_tiles.is_empty());
        assert_eq!(
            no_floors.feedback().unwrap().reason,
            PlacementRejectReason::NoCompletedFloor
        );

        let bypassed = build_wall_placement_plan(&area, &world_map, &HashSet::new(), true);
        assert_eq!(bypassed.valid_tiles, vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn wall_placement_plan_rejects_non_linear_area_structurally() {
        let world_map = WorldMap::default();
        let area = area_for_grids((0, 0), (1, 1));
        let plan = build_wall_placement_plan(
            &area,
            &world_map,
            &HashSet::from([(0, 0), (1, 0), (0, 1), (1, 1)]),
            false,
        );
        assert!(plan.valid_tiles.is_empty());
        assert_eq!(
            plan.feedback().unwrap().reason,
            PlacementRejectReason::NotStraightLine
        );
    }
}
