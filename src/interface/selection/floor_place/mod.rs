//! Floor and wall construction drag-drop placement system

mod floor_apply;
mod validation;
mod wall_apply;

use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::{PlacementFailureTooltip, UiInputState};
use crate::systems::command::area_selection::wall_line_area;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;

use floor_apply::apply_floor_placement;
use wall_apply::apply_wall_placement;

pub fn floor_placement_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    q_existing_floor_tiles: Query<&FloorTileBlueprint>,
    q_floor_buildings: Query<(&Building, &Transform)>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: ResMut<WorldMap>,
    mut placement_failure_tooltip: ResMut<PlacementFailureTooltip>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let (is_floor_mode, start_pos_opt) = match task_context.0 {
        TaskMode::FloorPlace(start_pos_opt) => (true, start_pos_opt),
        TaskMode::WallPlace(start_pos_opt) => (false, start_pos_opt),
        _ => return,
    };

    let Some(world_pos) = super::placement_common::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    // Start drag
    if buttons.just_pressed(MouseButton::Left) {
        task_context.0 = if is_floor_mode {
            TaskMode::FloorPlace(Some(snapped_pos))
        } else {
            TaskMode::WallPlace(Some(snapped_pos))
        };
        return;
    }

    // Complete placement
    if buttons.just_released(MouseButton::Left) {
        if let Some(start_pos) = start_pos_opt {
            if is_floor_mode {
                let area = TaskArea::from_points(start_pos, snapped_pos);
                let existing_floor_tile_grids: HashSet<(i32, i32)> = q_existing_floor_tiles
                    .iter()
                    .map(|tile| tile.grid_pos)
                    .collect();
                let existing_floor_building_grids: HashSet<(i32, i32)> = q_floor_buildings
                    .iter()
                    .filter_map(|(building, transform)| {
                        (building.kind == BuildingType::Floor)
                            .then(|| WorldMap::world_to_grid(transform.translation.truncate()))
                    })
                    .collect();
                apply_floor_placement(
                    &mut commands,
                    &world_map,
                    &area,
                    &existing_floor_tile_grids,
                    &existing_floor_building_grids,
                    &mut placement_failure_tooltip,
                );
            } else {
                let area = wall_line_area(start_pos, snapped_pos);
                let existing_floor_building_grids: HashSet<(i32, i32)> = q_floor_buildings
                    .iter()
                    .filter_map(|(building, transform)| {
                        (building.kind == BuildingType::Floor)
                            .then(|| WorldMap::world_to_grid(transform.translation.truncate()))
                    })
                    .collect();
                apply_wall_placement(
                    &mut commands,
                    &mut world_map,
                    &area,
                    &existing_floor_building_grids,
                    &mut placement_failure_tooltip,
                );
            }

            // Reset mode (continue placing if shift held - TODO)
            task_context.0 = if is_floor_mode {
                TaskMode::FloorPlace(None)
            } else {
                TaskMode::WallPlace(None)
            };
        }
        return;
    }

    // Cancel (right click)
    if buttons.just_pressed(MouseButton::Right) {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
    }
}
