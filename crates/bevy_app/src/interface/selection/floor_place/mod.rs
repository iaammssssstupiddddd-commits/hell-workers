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
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::systems::jobs::Building;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;
use hw_ui::selection::PlacementFeedbackState;
use std::collections::HashSet;

use input::{
    FloorQueryGroup, FloorReleaseData, FloorReleaseState, handle_cancel, handle_drag_start,
    handle_release,
};
use validation::{
    build_floor_placement_plan, build_wall_placement_plan, existing_floor_building_grids,
};

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
    pub placement_feedback: ResMut<'w, PlacementFeedbackState>,
    pub real_time: Res<'w, Time<Real>>,
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
    if input.ui_input_state.world_input_blocked() {
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

    if handle_drag_start(
        &input.buttons,
        &mut context.task_context.0,
        is_floor_mode,
        snapped_pos,
    ) {
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

    let now = context.real_time.elapsed();
    if handle_release(
        &input.buttons,
        data,
        &fq,
        &mut commands,
        &mut world_map,
        FloorReleaseState {
            placement_feedback: &mut context.placement_feedback,
            task_mode: &mut context.task_context.0,
            now,
        },
    ) {
        return;
    }

    let _ = handle_cancel(
        &input.buttons,
        &mut context.task_context.0,
        &mut context.next_play_mode,
    );
}

#[derive(SystemParam)]
pub struct FloorPlacePreviewContext<'w, 's> {
    pub task_context: Res<'w, TaskContext>,
    pub placement_feedback: ResMut<'w, PlacementFeedbackState>,
    pub q_existing_floor_tiles: Query<'w, 's, &'static FloorTileBlueprint>,
    pub q_floor_buildings: Query<'w, 's, (&'static Building, &'static Transform)>,
    pub debug_instant_build: Res<'w, crate::DebugInstantBuild>,
}

pub fn floor_placement_preview_system(
    input: FloorPlaceInput,
    mut context: FloorPlacePreviewContext,
    world_map: crate::world::map::WorldMapRead,
) {
    if input.ui_input_state.world_input_blocked() {
        return;
    }
    let (is_floor_mode, start_pos) = match context.task_context.0 {
        TaskMode::FloorPlace(Some(start)) => (true, start),
        TaskMode::WallPlace(Some(start)) => (false, start),
        _ => return,
    };
    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);
    let existing_floor_building_grids = existing_floor_building_grids(&context.q_floor_buildings);
    let plan = if is_floor_mode {
        let area = crate::systems::command::TaskArea::from_points(start_pos, snapped_pos);
        let existing_floor_tile_grids: HashSet<_> = context
            .q_existing_floor_tiles
            .iter()
            .map(|tile| tile.grid_pos)
            .collect();
        build_floor_placement_plan(
            &area,
            world_map.as_ref(),
            &existing_floor_tile_grids,
            &existing_floor_building_grids,
        )
    } else {
        let area = crate::systems::command::wall_line_area(start_pos, snapped_pos);
        build_wall_placement_plan(
            &area,
            world_map.as_ref(),
            &existing_floor_building_grids,
            context.debug_instant_build.0,
        )
    };
    context.placement_feedback.set_live_area_plan(&plan);
}
