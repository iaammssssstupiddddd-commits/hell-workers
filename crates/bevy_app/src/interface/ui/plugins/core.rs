use crate::interface::selection::SelectedEntity;
use crate::interface::selection::blueprint_placement;
use crate::interface::selection::building_move_preview_system;
use crate::interface::selection::building_move_system;
use crate::interface::selection::floor_placement_preview_system;
use crate::interface::selection::floor_placement_system;
use crate::interface::selection::soul_spa_place_input_system;
use crate::interface::selection::{
    cleanup_selection_references_system, clear_companion_state_outside_build_mode,
    update_hover_entity, update_selection_indicator,
};
use crate::interface::ui::interaction::handle_ui_intent;
use crate::interface::ui::vignette::update_vignette_system;
use crate::systems::GameSystemSet;
use crate::systems::command::StockpilePolicyRangeEditState;
use crate::systems::time::game_time_system;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_ui::notifications::NotificationSystemSet;
use hw_ui::selection::PlacementFeedbackSet;

pub type UiCorePlugin = hw_ui::plugins::core::UiCorePlugin;

pub fn ui_core_plugin() -> UiCorePlugin {
    UiCorePlugin::new(register_ui_core_plugin_systems)
}

fn register_ui_core_plugin_systems(app: &mut App) {
    configure_placement_feedback_sets(app);
    app.init_resource::<StockpilePolicyRangeEditState>();
    app.add_systems(
        Update,
        (
            cleanup_selection_references_system,
            update_hover_entity,
            crate::interface::ui::update_move_plant_hover_target_system,
            crate::interface::ui::hover_action_button_system,
        )
            .chain()
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        (
            clear_companion_state_outside_build_mode,
            update_selection_indicator,
        )
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        (
            building_move_preview_system.run_if(in_state(PlayMode::BuildingMove)),
            floor_placement_preview_system.run_if(in_state(PlayMode::FloorPlace)),
        )
            .in_set(PlacementFeedbackSet::Produce),
    )
    .add_systems(
        Update,
        (
            blueprint_placement.run_if(in_state(PlayMode::BuildingPlace)),
            floor_placement_system.run_if(in_state(PlayMode::FloorPlace)),
            building_move_system.run_if(in_state(PlayMode::BuildingMove)),
            soul_spa_place_input_system.run_if(in_state(PlayMode::TaskDesignation)),
        )
            .in_set(PlacementFeedbackSet::Commit),
    )
    .add_systems(
        Update,
        (
            crate::interface::ui::ui_interaction_system,
            crate::interface::ui::interaction::handle_text_input_intents_system,
            crate::interface::ui::panels::task_list::task_dashboard_action_button_system,
            handle_ui_intent,
            hw_logistics::apply_stockpile_policy_change_requests_system
                .before(NotificationSystemSet::Adapt),
            crate::interface::ui::panels::task_list::apply_task_action_intents_system
                .before(NotificationSystemSet::Adapt),
            crate::interface::ui::menu_visibility_system,
            hw_ui::interaction::update_pause_menu_visibility_system,
            hw_ui::interaction::update_settings_panel_visibility,
            hw_ui::interaction::sync_settings_slider_thumbs_system,
            hw_ui::interaction::sync_settings_checkmarks_system,
            crate::interface::ui::update_mode_text_system,
            crate::interface::ui::update_area_edit_preview_ui_system,
        )
            .chain()
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        (
            crate::interface::ui::context_menu_system,
            crate::interface::ui::task_summary_ui_system,
            crate::interface::ui::update_operation_dialog_system
                .run_if(|selected: Res<SelectedEntity>| selected.0.is_some()),
            game_time_system,
            crate::interface::ui::update_fps_display_system,
            crate::interface::ui::update_dream_pool_display_system,
            crate::interface::ui::update_dream_loss_popup_ui_system,
            crate::interface::ui::update_speed_button_highlight_system,
            update_vignette_system,
        )
            .chain()
            .after(crate::interface::ui::update_area_edit_preview_ui_system)
            .in_set(GameSystemSet::Interface),
    );
}

fn configure_placement_feedback_sets(app: &mut App) {
    app.configure_sets(
        Update,
        (
            PlacementFeedbackSet::Produce,
            PlacementFeedbackSet::Present,
            PlacementFeedbackSet::Commit,
        )
            .chain()
            .in_set(GameSystemSet::Interface),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct PlacementOrder(Vec<&'static str>);

    fn record_produce(mut order: ResMut<PlacementOrder>) {
        order.0.push("produce");
    }

    fn record_present(mut order: ResMut<PlacementOrder>) {
        order.0.push("present");
    }

    fn record_commit(mut order: ResMut<PlacementOrder>) {
        order.0.push("commit");
    }

    #[test]
    fn placement_feedback_sets_keep_produce_present_commit_order() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PlacementOrder>();
        configure_placement_feedback_sets(&mut app);
        app.add_systems(Update, record_produce.in_set(PlacementFeedbackSet::Produce))
            .add_systems(Update, record_present.in_set(PlacementFeedbackSet::Present))
            .add_systems(Update, record_commit.in_set(PlacementFeedbackSet::Commit));

        app.update();

        assert_eq!(
            app.world().resource::<PlacementOrder>().0,
            vec!["produce", "present", "commit"]
        );
    }
}
