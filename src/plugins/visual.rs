//! ビジュアル関連のプラグイン

use crate::entities::familiar::{familiar_animation_system, update_familiar_range_indicator};
use crate::game_state::PlayMode;
use crate::systems::GameSystemSet;
use crate::systems::command::{
    area_edit_handles_visual_system, area_selection_indicator_system,
    sync_designation_indicator_system, update_designation_indicator_system,
};
use crate::systems::jobs::building_completion_system;
use crate::systems::logistics::resource_count_display_system;
use crate::systems::soul_ai::visual::gathering::{
    gathering_debug_visualization_system, gathering_visual_update_system,
};
use crate::systems::soul_ai::visual::idle::idle_visual_system;
use crate::systems::soul_ai::visual::vitals::familiar_hover_visualization_system;
use crate::systems::visual::blueprint::{
    attach_blueprint_visual_system, blueprint_pulse_animation_system,
    blueprint_scale_animation_system, building_bounce_animation_system,
    cleanup_material_display_system, cleanup_progress_bars_system, material_delivery_vfx_system,
    spawn_material_display_system, spawn_progress_bar_system, spawn_worker_indicators_system,
    sync_progress_bar_position_system as bp_sync_progress_bar_position_system,
    update_blueprint_visual_system, update_completion_text_system, update_delivery_popup_system,
    update_material_counter_system,
    update_progress_bar_fill_system as bp_update_progress_bar_fill_system,
    update_worker_indicators_system,
};
use crate::systems::visual::dream::{
    dream_icon_absorb_system, dream_particle_spawn_system, dream_particle_update_system,
    dream_popup_spawn_system, dream_popup_update_system, dream_trail_ghost_update_system,
    ensure_dream_visual_state_system, rest_area_dream_particle_spawn_system,
    ui_particle_merge_system, ui_particle_update_system,
};
use crate::systems::visual::fade::fade_out_system;
use crate::systems::visual::floor_construction::{
    manage_floor_curing_progress_bars_system, sync_floor_tile_bone_visuals_system,
    update_floor_curing_progress_bars_system, update_floor_tile_visuals_system,
};
use crate::systems::visual::gather::{
    attach_resource_visual_system, cleanup_resource_visual_system, spawn_gather_indicators_system,
    update_gather_indicators_system, update_resource_visual_system,
};
use crate::systems::visual::haul::{
    spawn_carrying_item_system, update_carrying_item_system, update_drop_popup_system,
    wheelbarrow_follow_system,
};
use crate::systems::visual::mud_mixer::update_mud_mixer_visual_system;
use crate::systems::visual::soul::{
    progress_bar_system, soul_status_visual_system, sync_progress_bar_position_system,
    task_link_system, update_progress_bar_fill_system,
};
use crate::systems::visual::speech::SpeechPlugin;
use crate::systems::visual::tank::update_tank_visual_system;
use crate::systems::visual::task_area_visual::{
    TaskAreaMaterial, update_task_area_material_system,
};
use crate::systems::visual::wall_connection::WallConnectionPlugin;
use crate::systems::utils::floating_text::update_all_floating_texts_system;
use crate::systems::visual::wall_construction::{
    manage_wall_progress_bars_system, update_wall_progress_bars_system,
    update_wall_tile_visuals_system,
};

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpeechPlugin);
        app.add_plugins(WallConnectionPlugin);
        app.add_plugins(Material2dPlugin::<TaskAreaMaterial>::default());
        // Blueprint visual systems (separate to avoid tuple limit)
        app.add_systems(
            Update,
            (
                attach_blueprint_visual_system,
                update_blueprint_visual_system,
                blueprint_pulse_animation_system,
                blueprint_scale_animation_system,
                spawn_progress_bar_system,
                bp_update_progress_bar_fill_system,
                bp_sync_progress_bar_position_system,
                cleanup_progress_bars_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Existing visual systems
        app.add_systems(
            Update,
            (
                progress_bar_system,
                update_progress_bar_fill_system,
                sync_progress_bar_position_system,
                soul_status_visual_system,
                task_link_system,
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

        // Blueprint detail systems (Phase 4)
        app.add_systems(
            Update,
            (
                spawn_material_display_system,
                update_material_counter_system,
                material_delivery_vfx_system,
                update_delivery_popup_system,
                update_completion_text_system,
                building_bounce_animation_system,
                spawn_worker_indicators_system,
                update_worker_indicators_system,
                cleanup_material_display_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Gather visual systems (Phase 1: 伐採ワーカーインジケータ)
        app.add_systems(
            Update,
            (
                spawn_gather_indicators_system,
                update_gather_indicators_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Resource highlight systems (Phase 2: リソースハイライト)
        app.add_systems(
            Update,
            (
                attach_resource_visual_system,
                update_resource_visual_system,
                cleanup_resource_visual_system,
                fade_out_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Area indicators
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
                    |state: Res<State<crate::game_state::PlayMode>>| match state.get() {
                        crate::game_state::PlayMode::Normal
                        | crate::game_state::PlayMode::BuildingPlace
                        | crate::game_state::PlayMode::TaskDesignation => true,
                        _ => false,
                    },
                ),
        );

        // Haul visual systems (Phase 3: 運搬ビジュアル)
        app.add_systems(
            Update,
            (
                spawn_carrying_item_system,
                update_carrying_item_system,
                update_drop_popup_system,
                wheelbarrow_follow_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        app.add_systems(
            Update,
            (
                ensure_dream_visual_state_system,
                dream_particle_spawn_system,
                rest_area_dream_particle_spawn_system,
                dream_popup_spawn_system,
                dream_particle_update_system,
                dream_popup_update_system,
                ui_particle_update_system,
                ui_particle_merge_system,
                dream_trail_ghost_update_system,
                dream_icon_absorb_system,
                update_all_floating_texts_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // More visual systems
        app.add_systems(
            Update,
            (
                idle_visual_system,
                familiar_animation_system,
                update_familiar_range_indicator,
                update_tank_visual_system,
                update_mud_mixer_visual_system,
                manage_floor_curing_progress_bars_system,
                update_floor_curing_progress_bars_system,
                update_floor_tile_visuals_system,
                sync_floor_tile_bone_visuals_system,
                manage_wall_progress_bars_system,
                update_wall_progress_bars_system,
                update_wall_tile_visuals_system,
                familiar_hover_visualization_system,
                gathering_visual_update_system,
                gathering_debug_visualization_system,
                update_task_area_material_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );
    }
}
