use bevy::prelude::*;
use hw_core::GameSettings;
use hw_ui::UiIntent;

use super::super::intent_context::IntentUiQueries;
use super::begin_overlay_open;
use crate::systems::save::SaveRecoveryMode;
use crate::systems::save::catalog::AutosaveGenerationLimit;
use crate::systems::save::catalog_ui::{
    begin_load_confirm, begin_overwrite_confirm, cancel_confirm, close_catalog, open_load_catalog,
    open_save_catalog, request_catalog_load, request_manual_save_for_slot, request_overwrite_save,
};

pub(crate) fn handle(intent: UiIntent, ui: &mut IntentUiQueries, settings: &GameSettings) {
    let recovery = *ui.save_recovery;
    let generations = AutosaveGenerationLimit(settings.normalized_autosave_generations());
    let seed = ui.worldgen_seed();

    match intent {
        UiIntent::SaveGame => {
            if recovery == SaveRecoveryMode::RecoveryFailed {
                return;
            }
            begin_overlay_open(&mut ui.input_focus);
            open_save_catalog(
                &mut ui.save_catalog_ui,
                &mut ui.save_dialog_session,
                &mut ui.save_catalog,
                &ui.save_storage_root,
                generations,
                seed,
            );
            info!("Save catalog opened");
        }
        UiIntent::RequestLoadGame => {
            begin_overlay_open(&mut ui.input_focus);
            open_load_catalog(
                &mut ui.save_catalog_ui,
                &mut ui.save_dialog_session,
                &mut ui.save_catalog,
                &ui.save_storage_root,
                generations,
                seed,
                recovery,
            );
            info!("Load catalog opened");
        }
        UiIntent::SelectSaveCatalogSlot { slot, session } => {
            let Some(entry) = ui.save_catalog.entry(slot) else {
                return;
            };
            if !entry.capabilities.can_manual_save {
                return;
            }
            if entry.revision.exists {
                let _ = begin_overwrite_confirm(&mut ui.save_catalog_ui, slot, session);
            } else {
                let _ = request_manual_save_for_slot(
                    &mut ui.save_load_state,
                    &ui.save_catalog,
                    &ui.save_catalog_ui,
                    slot,
                    session,
                );
            }
        }
        UiIntent::ConfirmSaveCatalogSlot { slot, session } => {
            let _ = request_overwrite_save(
                &mut ui.save_load_state,
                &ui.save_catalog,
                &ui.save_catalog_ui,
                slot,
                session,
            );
        }
        UiIntent::SelectLoadCatalogSlot { slot, session } => {
            let Some(entry) = ui.save_catalog.entry(slot) else {
                return;
            };
            if !entry.capabilities.can_load {
                return;
            }
            let _ = begin_load_confirm(&mut ui.save_catalog_ui, slot, session);
        }
        UiIntent::ConfirmLoadCatalogSlot { slot, session } => {
            let _ = request_catalog_load(
                &mut ui.save_load_state,
                &ui.save_catalog,
                &ui.save_catalog_ui,
                recovery,
                slot,
                session,
            );
        }
        UiIntent::CancelSaveCatalogConfirm => {
            let session = ui.save_catalog_ui.session;
            let _ = cancel_confirm(&mut ui.save_catalog_ui, session);
        }
        UiIntent::CancelLoadConfirm => {
            let session = ui.save_catalog_ui.session;
            if !cancel_confirm(&mut ui.save_catalog_ui, session)
                && recovery != SaveRecoveryMode::RecoveryFailed
            {
                close_catalog(&mut ui.save_catalog_ui);
            }
        }
        UiIntent::CloseSaveCatalog if recovery != SaveRecoveryMode::RecoveryFailed => {
            close_catalog(&mut ui.save_catalog_ui);
        }
        _ => {}
    }
}
