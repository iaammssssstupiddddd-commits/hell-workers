use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::UiIntent;

use super::handlers;
use super::intent_context::{
    IntentFamiliarQueries, IntentModeCtx, IntentSelectionCtx, IntentUiQueries,
};
use crate::FamiliarOperationMaxSoulChangedEvent;

#[derive(SystemParam)]
pub(crate) struct IntentSettingsCtx<'w> {
    settings: ResMut<'w, hw_core::GameSettings>,
    debug_visible: ResMut<'w, crate::DebugVisible>,
    config_store: ResMut<'w, GizmoConfigStore>,
}

pub(crate) fn handle_ui_intent(
    mut ui_intents: MessageReader<UiIntent>,
    mut mode_ctx: IntentModeCtx,
    mut selection_ctx: IntentSelectionCtx,
    mut familiar_queries: IntentFamiliarQueries,
    mut ui_queries: IntentUiQueries,
    mut ev_max_soul_changed: MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    mut settings_ctx: IntentSettingsCtx,
) {
    for intent in ui_intents.read().cloned() {
        let should_save_settings = match intent {
            UiIntent::InspectEntity(_) | UiIntent::ClearInspectPin => {
                handlers::handle_selection(intent, &mut selection_ctx);
                false
            }
            UiIntent::ToggleArchitect
            | UiIntent::ToggleOrders
            | UiIntent::ToggleZones
            | UiIntent::ToggleDream => {
                handlers::handle_toggle(intent, &mut mode_ctx);
                false
            }
            UiIntent::SelectBuild(_)
            | UiIntent::SelectFloorPlace
            | UiIntent::SelectZone(_)
            | UiIntent::RemoveZone(_)
            | UiIntent::SelectTaskMode(_)
            | UiIntent::SelectAreaTask
            | UiIntent::SelectDreamPlanting => {
                handlers::handle_mode_select(
                    intent,
                    &mut mode_ctx,
                    &mut selection_ctx,
                    &familiar_queries,
                );
                false
            }
            UiIntent::OpenOperationDialog | UiIntent::CloseDialog => {
                handlers::handle_dialog(intent, &mut ui_queries);
                false
            }
            UiIntent::AdjustFatigueThreshold(_)
            | UiIntent::AdjustMaxControlledSoul(_)
            | UiIntent::AdjustMaxControlledSoulFor(..) => {
                handlers::handle_familiar_settings(
                    intent,
                    &mut selection_ctx,
                    &mut familiar_queries,
                    &mut ui_queries,
                    &mut ev_max_soul_changed,
                );
                false
            }
            UiIntent::TogglePause | UiIntent::SetTimeSpeed(_) => {
                handlers::handle_time(intent, &mut mode_ctx.time);
                false
            }
            UiIntent::SaveGame
            | UiIntent::RequestLoadGame
            | UiIntent::ConfirmLoadGame
            | UiIntent::CancelLoadConfirm => {
                handlers::handle_save_game(intent, &mut ui_queries);
                false
            }
            UiIntent::ToggleSettings
            | UiIntent::CloseSettings
            | UiIntent::SetUiScale(_)
            | UiIntent::SetCameraPanSpeed(_)
            | UiIntent::SetCameraMousePanEnabled(_)
            | UiIntent::SetDefaultTimeSpeed(_)
            | UiIntent::SetDebugGizmosEnabled(_)
            | UiIntent::SetFpsDisplayEnabled(_) => handlers::handle_settings(
                intent,
                &mut settings_ctx.settings,
                &mut mode_ctx.menu_state,
                &mut settings_ctx.debug_visible,
                &mut settings_ctx.config_store,
            ),
            UiIntent::ToggleDoorLock(_)
            | UiIntent::SelectArchitectCategory(_)
            | UiIntent::MovePlantBuilding(_) => false,
        };

        handlers::save_if_requested(should_save_settings, &settings_ctx.settings);
    }
}
