use bevy::prelude::*;
use hw_core::game_state::TimeSpeed;
use hw_ui::UiIntent;

use super::super::intent_context::{IntentSelectionCtx, IntentUiQueries};

pub(crate) fn handle_selection(intent: UiIntent, ctx: &mut IntentSelectionCtx<'_>) {
    match intent {
        UiIntent::InspectEntity(entity) => {
            ctx.selected_entity.0 = Some(entity);
            ctx.info_panel_pin.entity = Some(entity);
        }
        UiIntent::ClearInspectPin => {
            ctx.info_panel_pin.entity = None;
        }
        _ => {}
    }
}

pub(crate) fn handle_dialog(intent: UiIntent, ui_queries: &mut IntentUiQueries<'_, '_>) {
    match intent {
        UiIntent::OpenOperationDialog => {
            hw_ui::interaction::dialog::open_operation_dialog(&mut ui_queries.q_dialog);
        }
        UiIntent::CloseDialog => {
            hw_ui::interaction::dialog::close_operation_dialog(&mut ui_queries.q_dialog);
        }
        _ => {}
    }
}

pub(crate) fn handle_time(intent: UiIntent, time: &mut Time<Virtual>) {
    match intent {
        UiIntent::TogglePause => {
            if time.is_paused() {
                time.unpause();
            } else {
                time.pause();
            }
        }
        UiIntent::SetTimeSpeed(speed) => match speed {
            TimeSpeed::Paused => time.pause(),
            TimeSpeed::Normal => {
                time.unpause();
                time.set_relative_speed(1.0);
            }
            TimeSpeed::Fast => {
                time.unpause();
                time.set_relative_speed(2.0);
            }
            TimeSpeed::Super => {
                time.unpause();
                time.set_relative_speed(4.0);
            }
        },
        _ => {}
    }
}
