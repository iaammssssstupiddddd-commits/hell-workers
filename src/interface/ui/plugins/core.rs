use crate::game_state::PlayMode;
use crate::interface::selection::SelectedEntity;
use crate::interface::selection::blueprint_placement;
use crate::interface::selection::{
    cleanup_selection_references_system, clear_companion_state_outside_build_mode,
    update_hover_entity, update_selection_indicator,
};
use crate::interface::ui::{
    context_menu_system, menu_visibility_system, task_summary_ui_system, ui_interaction_system,
    ui_keyboard_shortcuts_system, update_area_edit_preview_ui_system,
    update_dream_pool_display_system, update_fps_display_system, update_mode_text_system,
    update_operation_dialog_system,
};
use crate::systems::GameSystemSet;
use crate::systems::time::game_time_system;
use bevy::prelude::*;

pub struct UiCorePlugin;

impl Plugin for UiCorePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                update_hover_entity,
                clear_companion_state_outside_build_mode,
                cleanup_selection_references_system,
                update_selection_indicator,
                blueprint_placement.run_if(in_state(PlayMode::BuildingPlace)),
                ui_keyboard_shortcuts_system,
                ui_interaction_system,
                menu_visibility_system,
                update_mode_text_system,
                update_area_edit_preview_ui_system,
            )
                .chain()
                .in_set(GameSystemSet::Interface),
        )
        .add_systems(
            Update,
            (
                context_menu_system,
                task_summary_ui_system,
                update_operation_dialog_system
                    .run_if(|selected: Res<SelectedEntity>| selected.0.is_some()),
                game_time_system,
                update_fps_display_system,
                update_dream_pool_display_system,
            )
                .in_set(GameSystemSet::Interface),
        );
    }
}
