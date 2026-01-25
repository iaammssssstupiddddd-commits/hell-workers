//! ビジュアル関連のプラグイン

use crate::entities::familiar::{familiar_animation_system, update_familiar_range_indicator};
use crate::game_state::PlayMode;
use crate::systems::GameSystemSet;
use crate::systems::command::{
    area_selection_indicator_system, designation_visual_system, familiar_command_visual_system,
    task_area_indicator_system, update_designation_indicator_system,
};
use crate::systems::jobs::building_completion_system;
use crate::systems::logistics::resource_count_display_system;
use crate::systems::soul_ai::idle::visual::idle_visual_system;
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
use crate::systems::visual::gather::{
    attach_resource_visual_system, cleanup_resource_visual_system, spawn_gather_indicators_system,
    update_gather_indicators_system, update_resource_visual_system,
};
use crate::systems::visual::haul::{
    spawn_carrying_item_system, update_carrying_item_system, update_drop_popup_system,
};
use crate::systems::visual::soul::{
    progress_bar_system, soul_status_visual_system, sync_progress_bar_position_system,
    task_link_system, update_progress_bar_fill_system,
};
use crate::systems::visual::speech::SpeechPlugin;
use crate::systems::visual::tank::update_tank_visual_system;
use bevy::prelude::*;

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpeechPlugin);
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
                task_area_indicator_system,
                area_selection_indicator_system.run_if(in_state(PlayMode::TaskDesignation)),
                designation_visual_system,
                update_designation_indicator_system,
                familiar_command_visual_system,
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
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Haul visual systems (Phase 3: 運搬ビジュアル)
        app.add_systems(
            Update,
            (
                spawn_carrying_item_system,
                update_carrying_item_system,
                update_drop_popup_system,
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
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );
    }
}
