use crate::systems::command::wall_line_area;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::Building;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_ui::selection::PlacementFeedbackState;
use std::collections::HashSet;

use super::floor_apply::apply_floor_placement;
use super::validation::{
    build_floor_placement_plan, build_wall_placement_plan, existing_floor_building_grids,
};
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

pub(super) struct FloorReleaseState<'a> {
    pub placement_feedback: &'a mut PlacementFeedbackState,
    pub task_mode: &'a mut TaskMode,
    pub now: std::time::Duration,
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
    state: FloorReleaseState<'_>,
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
            let plan = build_floor_placement_plan(
                &area,
                world_map,
                &existing_floor_tile_grids,
                &existing_floor_building_grids,
            );
            if plan.valid_tiles.is_empty() {
                if let Some(feedback) = plan.feedback() {
                    warn!("Floor placement rejected: {}", feedback.body());
                    state
                        .placement_feedback
                        .show_recent_failure(feedback, state.now);
                }
            } else {
                state.placement_feedback.clear_recent_failure();
                apply_floor_placement(commands, &area, &plan);
            }
        } else {
            let area = wall_line_area(start_pos, data.snapped_pos);
            let existing_floor_building_grids =
                existing_floor_building_grids(queries.q_floor_buildings);
            let plan = build_wall_placement_plan(
                &area,
                world_map,
                &existing_floor_building_grids,
                data.bypass_floor_check,
            );
            if plan.valid_tiles.is_empty() {
                if let Some(feedback) = plan.feedback() {
                    warn!("Wall placement rejected: {}", feedback.body());
                    state
                        .placement_feedback
                        .show_recent_failure(feedback, state.now);
                }
            } else {
                state.placement_feedback.clear_recent_failure();
                apply_wall_placement(commands, world_map, &area, &plan);
            }
        }

        // Reset mode (continue placing if shift held - TODO)
        *state.task_mode = if data.is_floor_mode {
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
