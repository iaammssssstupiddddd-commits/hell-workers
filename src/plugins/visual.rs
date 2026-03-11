//! ビジュアル関連のプラグイン

use crate::entities::familiar::{familiar_animation_system, update_familiar_range_indicator};
use crate::systems::GameSystemSet;
use crate::systems::command::{
    area_edit_handles_visual_system, area_selection_indicator_system,
    dream_tree_planting_preview_system, sync_designation_indicator_system,
    update_designation_indicator_system,
};
use crate::systems::jobs::building_completion_system;
use crate::systems::logistics::resource_count_display_system;
use crate::systems::room::sync_room_overlay_tiles_system;
use crate::systems::visual::floor_construction::{
    manage_floor_curing_progress_bars_system, sync_floor_tile_bone_visuals_system,
    update_floor_curing_progress_bars_system, update_floor_tile_visuals_system,
};
use crate::systems::visual::task_area_visual::update_task_area_material_system;
use crate::systems::visual::wall_construction::{
    manage_wall_progress_bars_system, update_wall_progress_bars_system,
    update_wall_tile_visuals_system,
};
use hw_core::game_state::PlayMode;
use hw_visual::soul::task_link_system;
use hw_visual::HwVisualPlugin;

use bevy::prelude::*;

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HwVisualPlugin);

        app.add_systems(
            Update,
            sync_room_overlay_tiles_system.in_set(GameSystemSet::Visual),
        );

        // Area indicators (app_contexts 依存のため root 残留)
        app.add_systems(
            Update,
            (
                crate::systems::command::task_area_indicator_system,
                area_edit_handles_visual_system,
                crate::systems::command::designation_visual_system,
                crate::systems::command::familiar_command_visual_system,
                crate::systems::visual::placement_ghost::placement_ghost_system,
            )
                .in_set(GameSystemSet::Visual)
                .run_if(
                    |state: Res<State<hw_core::game_state::PlayMode>>| match state.get() {
                        PlayMode::Normal
                        | PlayMode::BuildingPlace
                        | PlayMode::TaskDesignation => true,
                        _ => false,
                    },
                ),
        );

        app.add_systems(
            Update,
            dream_tree_planting_preview_system.in_set(GameSystemSet::Visual),
        );

        // task_link は DebugVisible（root 専有リソース）で条件付き実行
        app.add_systems(
            Update,
            task_link_system
                .run_if(|debug: Res<crate::DebugVisible>| debug.0)
                .in_set(GameSystemSet::Visual),
        );

        // root 残留の visual systems（jobs / logistics / soul_ai / familiar 由来）
        app.add_systems(
            Update,
            (
                building_completion_system,
                area_selection_indicator_system.run_if(|play_mode: Res<State<PlayMode>>| {
                    matches!(
                        play_mode.get(),
                        PlayMode::TaskDesignation | PlayMode::FloorPlace
                    )
                }),
                update_designation_indicator_system,
                sync_designation_indicator_system,
                resource_count_display_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        app.add_systems(
            Update,
            (
                familiar_animation_system,
                update_familiar_range_indicator,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Floor / wall construction + task area visual（root 残留型に依存）
        app.add_systems(
            Update,
            (
                manage_floor_curing_progress_bars_system,
                update_floor_curing_progress_bars_system,
                update_floor_tile_visuals_system,
                sync_floor_tile_bone_visuals_system,
                manage_wall_progress_bars_system,
                update_wall_progress_bars_system,
                update_wall_tile_visuals_system,
                update_task_area_material_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );
    }
}

