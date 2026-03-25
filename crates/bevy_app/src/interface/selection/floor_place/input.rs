use crate::interface::ui::PlacementFailureTooltip;
use crate::systems::command::wall_line_area;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use std::collections::HashSet;

use super::floor_apply::apply_floor_placement;
use super::wall_apply::apply_wall_placement;

pub(super) struct FloorReleaseData {
    pub start_pos_opt: Option<Vec2>,
    pub is_floor_mode: bool,
    pub snapped_pos: Vec2,
    pub bypass_floor_check: bool,
}

pub(super) struct FloorQueryGroup<'a, 'w, 's> {
    pub q_existing_floor_tiles: &'a Query<'w, 's, &'static FloorTileBlueprint>,
    pub q_floor_buildings: &'a Query<'w, 's, (&'static Building, &'static Transform)>,
}

pub(super) fn handle_drag_start(
    buttons: &ButtonInput<MouseButton>,
    task_mode: &mut TaskMode,
    is_floor_mode: bool,
    snapped_pos: Vec2,
) -> bool {
    if !buttons.just_pressed(MouseButton::Left) {
        return false;
    }

    *task_mode = if is_floor_mode {
        TaskMode::FloorPlace(Some(snapped_pos))
    } else {
        TaskMode::WallPlace(Some(snapped_pos))
    };
    true
}

pub(super) fn handle_release(
    buttons: &ButtonInput<MouseButton>,
    data: FloorReleaseData,
    queries: &FloorQueryGroup<'_, '_, '_>,
    commands: &mut Commands,
    world_map: &mut WorldMap,
    placement_failure_tooltip: &mut PlacementFailureTooltip,
    task_mode: &mut TaskMode,
) -> bool {
    if !buttons.just_released(MouseButton::Left) {
        return false;
    }

    if let Some(start_pos) = data.start_pos_opt {
        if data.is_floor_mode {
            let area = TaskArea::from_points(start_pos, data.snapped_pos);
            let existing_floor_tile_grids: HashSet<(i32, i32)> = queries
                .q_existing_floor_tiles
                .iter()
                .map(|tile| tile.grid_pos)
                .collect();
            let existing_floor_building_grids =
                existing_floor_building_grids(queries.q_floor_buildings);
            apply_floor_placement(
                commands,
                world_map,
                &area,
                &existing_floor_tile_grids,
                &existing_floor_building_grids,
                placement_failure_tooltip,
            );
        } else {
            let area = wall_line_area(start_pos, data.snapped_pos);
            let existing_floor_building_grids =
                existing_floor_building_grids(queries.q_floor_buildings);
            apply_wall_placement(
                commands,
                world_map,
                &area,
                &existing_floor_building_grids,
                placement_failure_tooltip,
                data.bypass_floor_check,
            );
        }

        // Reset mode (continue placing if shift held - TODO)
        *task_mode = if data.is_floor_mode {
            TaskMode::FloorPlace(None)
        } else {
            TaskMode::WallPlace(None)
        };
    }

    true
}

pub(super) fn handle_cancel(
    buttons: &ButtonInput<MouseButton>,
    task_mode: &mut TaskMode,
    next_play_mode: &mut NextState<PlayMode>,
) -> bool {
    if !buttons.just_pressed(MouseButton::Right) {
        return false;
    }

    *task_mode = TaskMode::None;
    next_play_mode.set(PlayMode::Normal);
    true
}

fn existing_floor_building_grids(
    q_floor_buildings: &Query<(&Building, &Transform)>,
) -> HashSet<(i32, i32)> {
    q_floor_buildings
        .iter()
        .filter(|&(building, _transform)| building.kind == BuildingType::Floor ).map(|(_building, transform)| WorldMap::world_to_grid(transform.translation.truncate()))
        .collect()
}
