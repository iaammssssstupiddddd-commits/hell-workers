use hw_core::game_state::PlayMode;
use crate::interface::selection::SelectedEntity;
use crate::interface::selection::blueprint_placement;
use crate::interface::selection::building_move_preview_system;
use crate::interface::selection::building_move_system;
use crate::interface::selection::floor_placement_system;
use crate::interface::selection::{
    cleanup_selection_references_system, clear_companion_state_outside_build_mode,
    update_hover_entity, update_selection_indicator,
};
use crate::interface::ui::vignette::update_vignette_system;
use crate::interface::ui::interaction::handle_ui_intent;
use crate::systems::GameSystemSet;
use crate::systems::time::game_time_system;
use bevy::prelude::*;

pub type UiCorePlugin = hw_ui::plugins::core::UiCorePlugin;

pub fn ui_core_plugin() -> UiCorePlugin {
    UiCorePlugin::new(register_ui_core_plugin_systems)
}

fn register_ui_core_plugin_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            update_hover_entity,
            clear_companion_state_outside_build_mode,
            cleanup_selection_references_system,
            update_selection_indicator,
            crate::interface::ui::hover_action_button_system,
            blueprint_placement.run_if(in_state(PlayMode::BuildingPlace)),
            building_move_preview_system.run_if(in_state(PlayMode::BuildingMove)),
            floor_placement_system.run_if(in_state(PlayMode::FloorPlace)),
            building_move_system.run_if(in_state(PlayMode::BuildingMove)),
        )
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        (
            crate::interface::ui::ui_keyboard_shortcuts_system,
            crate::interface::ui::ui_interaction_system,
            handle_ui_intent,
            crate::interface::ui::arch_category_action_system,
            crate::interface::ui::move_plant_building_action_system,
            crate::interface::ui::door_lock_action_system,
        )
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        (
            crate::interface::ui::context_menu_system,
            crate::interface::ui::menu_visibility_system,
            crate::interface::ui::update_mode_text_system,
            crate::interface::ui::update_area_edit_preview_ui_system,
            crate::interface::ui::task_summary_ui_system,
            crate::interface::ui::update_operation_dialog_system.run_if(|selected: Res<SelectedEntity>| {
                selected.0.is_some()
            }),
            game_time_system,
            crate::interface::ui::update_fps_display_system,
            crate::interface::ui::update_dream_pool_display_system,
            crate::interface::ui::update_dream_loss_popup_ui_system,
            crate::interface::ui::update_speed_button_highlight_system,
            update_vignette_system,
        )
            .in_set(GameSystemSet::Interface),
    );
}
