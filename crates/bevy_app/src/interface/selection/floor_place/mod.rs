//! Floor and wall construction drag-drop placement system (root shell)
//!
//! Root shell: `TaskContext` / `WorldMap` 占有更新 / `FloorTileBlueprint` spawn に依存。
//! hw_ui / hw_jobs crate への移設には TaskContext / WorldMap の抽象化が必要であり、
//! 現段階では意図的に root に残す。純バリデーション API は hw_ui::selection::placement を参照。

mod floor_apply;
mod input;
mod validation;
mod wall_apply;

use crate::app_contexts::TaskContext;
use crate::interface::ui::{PlacementFailureTooltip, UiInputState};
use crate::systems::command::TaskMode;
use crate::systems::jobs::Building;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;

use input::{FloorQueryGroup, FloorReleaseData, handle_cancel, handle_drag_start, handle_release};

#[derive(SystemParam)]
pub struct FloorPlaceInput<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
}

#[derive(SystemParam)]
pub struct FloorPlaceContext<'w, 's> {
    pub task_context: ResMut<'w, TaskContext>,
    pub next_play_mode: ResMut<'w, NextState<PlayMode>>,
    pub placement_failure_tooltip: ResMut<'w, PlacementFailureTooltip>,
    pub q_existing_floor_tiles: Query<'w, 's, &'static FloorTileBlueprint>,
    pub q_floor_buildings: Query<'w, 's, (&'static Building, &'static Transform)>,
    pub debug_instant_build: Res<'w, crate::DebugInstantBuild>,
}

pub fn floor_placement_system(
    input: FloorPlaceInput,
    mut context: FloorPlaceContext,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    if input.ui_input_state.pointer_over_ui {
        return;
    }

    let (is_floor_mode, start_pos_opt) = match context.task_context.0 {
        TaskMode::FloorPlace(start_pos_opt) => (true, start_pos_opt),
        TaskMode::WallPlace(start_pos_opt) => (false, start_pos_opt),
        _ => return,
    };

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    if handle_drag_start(&input.buttons, &mut context.task_context.0, is_floor_mode, snapped_pos) {
        return;
    }

    let data = FloorReleaseData {
        start_pos_opt,
        is_floor_mode,
        snapped_pos,
        bypass_floor_check: context.debug_instant_build.0,
    };
    let fq = FloorQueryGroup {
        q_existing_floor_tiles: &context.q_existing_floor_tiles,
        q_floor_buildings: &context.q_floor_buildings,
    };

    if handle_release(
        &input.buttons,
        data,
        &fq,
        &mut commands,
        &mut world_map,
        &mut context.placement_failure_tooltip,
        &mut context.task_context.0,
    ) {
        return;
    }

    let _ = handle_cancel(&input.buttons, &mut context.task_context.0, &mut context.next_play_mode);
}
