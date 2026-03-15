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
use crate::systems::visual::building3d_cleanup::{
    cleanup_building_3d_visuals_system, sync_provisional_wall_material_system,
};
use crate::systems::visual::camera_sync::sync_camera3d_system;
use crate::systems::visual::character_proxy_3d::{
    cleanup_familiar_proxy_3d_system, cleanup_soul_proxy_3d_system,
    sync_familiar_proxy_3d_system, sync_soul_proxy_3d_system,
};
use crate::systems::visual::elevation_view::{
    ElevationViewState, elevation_view_input_system,
};
use crate::systems::visual::task_area_visual::update_task_area_material_system;
use hw_core::game_state::PlayMode;
use hw_visual::HwVisualPlugin;
use hw_visual::soul::task_link_system;

use bevy::prelude::*;

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HwVisualPlugin);

        app.init_resource::<ElevationViewState>();

        app.add_systems(
            Update,
            sync_camera3d_system.in_set(GameSystemSet::Visual),
        );

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
                        PlayMode::Normal | PlayMode::BuildingPlace | PlayMode::TaskDesignation => {
                            true
                        }
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
            (familiar_animation_system, update_familiar_range_indicator)
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // task area visual（root 残留型に依存）
        app.add_systems(
            Update,
            update_task_area_material_system.in_set(GameSystemSet::Visual),
        );

        // Building3dVisual クリーンアップ・マテリアル遷移
        app.add_systems(
            Update,
            (
                cleanup_building_3d_visuals_system,
                sync_provisional_wall_material_system,
            )
                .in_set(GameSystemSet::Visual),
        );

        // キャラクター3Dプロキシ同期・クリーンアップ
        app.add_systems(
            Update,
            (
                sync_soul_proxy_3d_system,
                sync_familiar_proxy_3d_system,
                cleanup_soul_proxy_3d_system,
                cleanup_familiar_proxy_3d_system,
            )
                .in_set(GameSystemSet::Visual),
        );

        // 矢視モード入力
        app.add_systems(
            Update,
            elevation_view_input_system.in_set(GameSystemSet::Input),
        );
    }
}
