use bevy::prelude::*;
use hw_ui::UiIntent;

use super::handlers;
use super::intent_context::{
    IntentFamiliarQueries, IntentModeCtx, IntentSelectionCtx, IntentUiQueries,
};
use crate::FamiliarOperationMaxSoulChangedEvent;

pub(crate) fn handle_ui_intent(
    mut ui_intents: MessageReader<UiIntent>,
    mut mode_ctx: IntentModeCtx,
    mut selection_ctx: IntentSelectionCtx,
    mut familiar_queries: IntentFamiliarQueries,
    mut ui_queries: IntentUiQueries,
    mut ev_max_soul_changed: MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    mut time: ResMut<Time<Virtual>>,
) {
    for intent in ui_intents.read().cloned() {
        match intent {
            UiIntent::InspectEntity(_) | UiIntent::ClearInspectPin => {
                handlers::handle_selection(intent, &mut selection_ctx);
            }
            UiIntent::ToggleArchitect
            | UiIntent::ToggleOrders
            | UiIntent::ToggleZones
            | UiIntent::ToggleDream => {
                handlers::handle_toggle(intent, &mut mode_ctx);
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
            }
            UiIntent::OpenOperationDialog | UiIntent::CloseDialog => {
                handlers::handle_dialog(intent, &mut ui_queries);
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
            }
            UiIntent::TogglePause | UiIntent::SetTimeSpeed(_) => {
                handlers::handle_time(intent, &mut time);
            }
            UiIntent::ToggleDoorLock(_)
            | UiIntent::SelectArchitectCategory(_)
            | UiIntent::MovePlantBuilding(_) => {
                // 専用システム側で扱うためここでは無視
            }
        }
    }
}
