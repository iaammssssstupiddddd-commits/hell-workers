use crate::game_state::PlayMode;
use crate::interface::ui::PlacementFailureTooltip;
use crate::systems::command::area_selection::wall_line_area;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

use super::floor_apply::apply_floor_placement;
use super::wall_apply::apply_wall_placement;

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
    start_pos_opt: Option<Vec2>,
    is_floor_mode: bool,
    snapped_pos: Vec2,
    q_existing_floor_tiles: &Query<&FloorTileBlueprint>,
    q_floor_buildings: &Query<(&Building, &Transform)>,
    commands: &mut Commands,
    world_map: &mut WorldMap,
    placement_failure_tooltip: &mut PlacementFailureTooltip,
    task_mode: &mut TaskMode,
) -> bool {
    if !buttons.just_released(MouseButton::Left) {
        return false;
    }

    if let Some(start_pos) = start_pos_opt {
        if is_floor_mode {
            let area = TaskArea::from_points(start_pos, snapped_pos);
            let existing_floor_tile_grids: HashSet<(i32, i32)> =
                q_existing_floor_tiles.iter().map(|tile| tile.grid_pos).collect();
            let existing_floor_building_grids = existing_floor_building_grids(q_floor_buildings);
            apply_floor_placement(
                commands,
                world_map,
                &area,
                &existing_floor_tile_grids,
                &existing_floor_building_grids,
                placement_failure_tooltip,
            );
        } else {
            let area = wall_line_area(start_pos, snapped_pos);
            let existing_floor_building_grids = existing_floor_building_grids(q_floor_buildings);
            apply_wall_placement(
                commands,
                world_map,
                &area,
                &existing_floor_building_grids,
                placement_failure_tooltip,
            );
        }

        // Reset mode (continue placing if shift held - TODO)
        *task_mode = if is_floor_mode {
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
        .filter_map(|(building, transform)| {
            (building.kind == BuildingType::Floor)
                .then(|| WorldMap::world_to_grid(transform.translation.truncate()))
        })
        .collect()
}
