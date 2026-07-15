use bevy::prelude::*;
use hw_ui::UiIntent;
use hw_ui::interaction::dialog::{close_load_confirm_dialog, open_load_confirm_dialog};

use super::super::intent_context::IntentUiQueries;
use crate::systems::save::SaveLoadState;

pub(crate) fn handle(intent: UiIntent, ui: &mut IntentUiQueries) {
    match intent {
        UiIntent::SaveGame => {
            if *ui.save_load_state == SaveLoadState::Idle {
                *ui.save_load_state = SaveLoadState::SaveRequested;
                info!("Save requested from pause menu");
            }
        }
        UiIntent::RequestLoadGame => {
            if !ui.save_path.as_path().exists() {
                warn!("No save file at {}", ui.save_path.as_path().display());
                return;
            }
            open_load_confirm_dialog(&mut ui.q_load_confirm);
        }
        UiIntent::ConfirmLoadGame => {
            close_load_confirm_dialog(&mut ui.q_load_confirm);
            if *ui.save_load_state == SaveLoadState::Idle {
                *ui.save_load_state = SaveLoadState::LoadRequested;
                info!("Load requested from confirmation dialog");
            }
        }
        UiIntent::CancelLoadConfirm => {
            close_load_confirm_dialog(&mut ui.q_load_confirm);
        }
        _ => {}
    }
}
