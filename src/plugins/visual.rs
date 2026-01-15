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
use crate::systems::soul_ai::idle::idle_visual_system;
use crate::systems::visuals::{
    progress_bar_system, soul_status_visual_system, sync_progress_bar_position_system,
    task_link_system, update_progress_bar_fill_system,
};
use bevy::prelude::*;

pub struct VisualPlugin;

impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
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
                idle_visual_system,
                familiar_animation_system,
                update_familiar_range_indicator,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );
    }
}
