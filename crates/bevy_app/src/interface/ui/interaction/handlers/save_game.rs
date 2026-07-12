use bevy::prelude::*;
use hw_ui::UiIntent;
use hw_ui::interaction::dialog::{close_load_confirm_dialog, open_load_confirm_dialog};

use super::super::intent_context::IntentUiQueries;
use crate::systems::save::{SAVE_FILE_PATH, SaveLoadState};

pub(crate) fn handle(intent: UiIntent, ui: &mut IntentUiQueries) {
    match intent {
        UiIntent::SaveGame => {
            if *ui.save_load_state == SaveLoadState::Idle {
                *ui.save_load_state = SaveLoadState::SaveRequested;
                info!("Save requested from pause menu");
            }
        }
        UiIntent::RequestLoadGame => {
            if !std::path::Path::new(SAVE_FILE_PATH).exists() {
                warn!("No save file at {SAVE_FILE_PATH}");
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
