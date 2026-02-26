//! Floor and wall construction drag-drop placement system

mod floor_apply;
mod input;
mod validation;
mod wall_apply;

use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::{PlacementFailureTooltip, UiInputState};
use crate::systems::command::TaskMode;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::Building;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use input::{handle_cancel, handle_drag_start, handle_release};

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

    if handle_drag_start(&buttons, &mut task_context.0, is_floor_mode, snapped_pos) {
        return;
    }

    if handle_release(
        &buttons,
        start_pos_opt,
        is_floor_mode,
        snapped_pos,
        &q_existing_floor_tiles,
        &q_floor_buildings,
        &mut commands,
        &mut world_map,
        &mut placement_failure_tooltip,
        &mut task_context.0,
    ) {
        return;
    }

    let _ = handle_cancel(&buttons, &mut task_context.0, &mut next_play_mode);
}
