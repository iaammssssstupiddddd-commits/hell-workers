//! Foreground save/load catalog owner (Track C2).

use bevy::prelude::*;
use hw_core::SaveSlotId;

use super::catalog::{
    AutosaveGenerationLimit, SaveCatalog, SaveExpectedTarget, SaveStorageRoot, refresh_save_catalog,
};
use super::saving::expected_target_from_revision;
use super::state::{
    LoadRequestOrigin, SaveDialogSession, SaveLoadFailureKind, SaveLoadOutcome,
    SaveLoadOutcomeSource, SaveLoadRequest, SaveLoadResult, SaveLoadState, SaveRecoveryMode,
    SaveRequestOrigin,
};

/// Catalog modal mode. Confirm states keep the underlying catalog capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveCatalogMode {
    #[default]
    Closed,
    SaveCatalog,
    OverwriteConfirm {
        slot: SaveSlotId,
    },
    LoadCatalog,
    RecoveryLoadCatalog,
    LoadConfirm {
        slot: SaveSlotId,
        /// The catalog owner that opened this confirmation. This is runtime
        /// state, not a caller-provided recovery capability.
        recovery: bool,
    },
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct SaveCatalogUi {
    pub mode: SaveCatalogMode,
    pub selected: Option<SaveSlotId>,
    pub session: u64,
}

impl SaveCatalogUi {
    pub fn is_open(&self) -> bool {
        !matches!(self.mode, SaveCatalogMode::Closed)
    }

    pub fn owns_session(&self, session: u64) -> bool {
        self.is_open() && self.session == session
    }

    pub fn is_recovery_catalog(&self) -> bool {
        matches!(
            self.mode,
            SaveCatalogMode::RecoveryLoadCatalog
                | SaveCatalogMode::LoadConfirm { recovery: true, .. }
        )
    }
}

pub(crate) fn open_save_catalog(
    ui: &mut SaveCatalogUi,
    session: &mut SaveDialogSession,
    catalog: &mut SaveCatalog,
    root: &SaveStorageRoot,
    generations: AutosaveGenerationLimit,
    worldgen_seed: Option<u64>,
) {
    ui.session = session.bump();
    ui.mode = SaveCatalogMode::SaveCatalog;
    ui.selected = None;
    refresh_save_catalog(catalog, root, worldgen_seed, generations);
}

pub(crate) fn open_load_catalog(
    ui: &mut SaveCatalogUi,
    session: &mut SaveDialogSession,
    catalog: &mut SaveCatalog,
    root: &SaveStorageRoot,
    generations: AutosaveGenerationLimit,
    worldgen_seed: Option<u64>,
    recovery: SaveRecoveryMode,
) {
    ui.session = session.bump();
    ui.mode = if recovery == SaveRecoveryMode::RecoveryFailed {
        SaveCatalogMode::RecoveryLoadCatalog
    } else {
        SaveCatalogMode::LoadCatalog
    };
    ui.selected = None;
    refresh_save_catalog(catalog, root, worldgen_seed, generations);
}

pub(crate) fn close_catalog(ui: &mut SaveCatalogUi) {
    *ui = SaveCatalogUi::default();
}

/// Releases or restores the foreground catalog only after the matching
/// one-shot request reaches a terminal outcome. Stale outcomes from a prior
/// dialog session cannot close a newer catalog.
pub(crate) fn settle_catalog_after_outcomes_system(
    mut outcomes: MessageReader<SaveLoadOutcome>,
    mut ui: ResMut<SaveCatalogUi>,
) {
    for outcome in outcomes.read() {
        let session = match outcome.source {
            SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog { dialog_session }) => {
                dialog_session
            }
            SaveLoadOutcomeSource::Load(origin) => origin.dialog_session(),
            SaveLoadOutcomeSource::Autosave => continue,
            SaveLoadOutcomeSource::Manual(SaveRequestOrigin::Autosave) => continue,
        };
        if !ui.owns_session(session) {
            continue;
        }

        match outcome.result {
            SaveLoadResult::Succeeded
            | SaveLoadResult::Failed(SaveLoadFailureKind::CommittedDurabilityUncertain) => {
                close_catalog(&mut ui);
            }
            SaveLoadResult::Failed(_) => {
                let _ = cancel_confirm(&mut ui, session);
            }
        }
    }
}

pub(crate) fn begin_overwrite_confirm(
    ui: &mut SaveCatalogUi,
    slot: SaveSlotId,
    session: u64,
) -> bool {
    if !ui.owns_session(session) || !matches!(ui.mode, SaveCatalogMode::SaveCatalog) {
        return false;
    }
    ui.selected = Some(slot);
    ui.mode = SaveCatalogMode::OverwriteConfirm { slot };
    true
}

pub(crate) fn begin_load_confirm(ui: &mut SaveCatalogUi, slot: SaveSlotId, session: u64) -> bool {
    if !ui.owns_session(session) {
        return false;
    }
    if !matches!(
        ui.mode,
        SaveCatalogMode::LoadCatalog | SaveCatalogMode::RecoveryLoadCatalog
    ) {
        return false;
    }
    let recovery = matches!(ui.mode, SaveCatalogMode::RecoveryLoadCatalog);
    ui.selected = Some(slot);
    ui.mode = SaveCatalogMode::LoadConfirm { slot, recovery };
    true
}

pub(crate) fn cancel_confirm(ui: &mut SaveCatalogUi, session: u64) -> bool {
    if !ui.owns_session(session) {
        return false;
    }
    ui.mode = match ui.mode {
        SaveCatalogMode::OverwriteConfirm { .. } => SaveCatalogMode::SaveCatalog,
        SaveCatalogMode::LoadConfirm { recovery: true, .. } => SaveCatalogMode::RecoveryLoadCatalog,
        SaveCatalogMode::LoadConfirm {
            recovery: false, ..
        } => SaveCatalogMode::LoadCatalog,
        _ => return false,
    };
    true
}

pub(crate) fn request_manual_save_for_slot(
    state: &mut SaveLoadState,
    catalog: &SaveCatalog,
    ui: &SaveCatalogUi,
    slot: SaveSlotId,
    session: u64,
) -> bool {
    if !ui.owns_session(session) || !matches!(ui.mode, SaveCatalogMode::SaveCatalog) {
        return false;
    }
    let Some(entry) = catalog.entry(slot) else {
        return false;
    };
    if !entry.capabilities.can_manual_save {
        return false;
    }
    if entry.revision.exists {
        return false;
    }
    let expected = expected_target_from_revision(entry.revision.clone());
    state.try_set(SaveLoadRequest::Save {
        origin: SaveRequestOrigin::ManualCatalog {
            dialog_session: session,
        },
        slot,
        expected_target: expected,
    })
}

pub(crate) fn request_overwrite_save(
    state: &mut SaveLoadState,
    catalog: &SaveCatalog,
    ui: &SaveCatalogUi,
    slot: SaveSlotId,
    session: u64,
) -> bool {
    if !ui.owns_session(session) {
        return false;
    }
    if !matches!(ui.mode, SaveCatalogMode::OverwriteConfirm { slot: confirmed } if confirmed == slot)
    {
        return false;
    }
    let Some(entry) = catalog.entry(slot) else {
        return false;
    };
    if !entry.capabilities.can_manual_save || !entry.revision.exists {
        return false;
    }
    state.try_set(SaveLoadRequest::Save {
        origin: SaveRequestOrigin::ManualCatalog {
            dialog_session: session,
        },
        slot,
        expected_target: SaveExpectedTarget::Exact(entry.revision.clone()),
    })
}

pub(crate) fn request_catalog_load(
    state: &mut SaveLoadState,
    catalog: &SaveCatalog,
    ui: &SaveCatalogUi,
    recovery: SaveRecoveryMode,
    slot: SaveSlotId,
    session: u64,
) -> bool {
    if !ui.owns_session(session) {
        return false;
    }
    let Some(entry) = catalog.entry(slot) else {
        return false;
    };
    if !entry.capabilities.can_load {
        return false;
    }
    let recovery_catalog = match ui.mode {
        SaveCatalogMode::LoadConfirm {
            slot: confirmed,
            recovery,
        } if confirmed == slot => recovery,
        _ => return false,
    };
    if recovery_catalog != (recovery == SaveRecoveryMode::RecoveryFailed) {
        return false;
    }

    let origin = match recovery {
        SaveRecoveryMode::RecoveryFailed => LoadRequestOrigin::RecoveryCatalog {
            dialog_session: session,
        },
        SaveRecoveryMode::Healthy => LoadRequestOrigin::NormalCatalog {
            dialog_session: session,
        },
    };

    state.try_set(SaveLoadRequest::Load { origin, slot })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::catalog::{
        SaveCatalogEntry, SaveContentStatus, SaveFileRevision, SaveSlotCapabilities,
    };
    use hw_core::SaveSlotRole;

    fn empty_manual_entry(slot: SaveSlotId) -> SaveCatalogEntry {
        SaveCatalogEntry {
            slot,
            role: SaveSlotRole::Manual,
            capabilities: SaveSlotCapabilities {
                can_manual_save: true,
                can_scheduler_save: false,
                can_load: false,
            },
            content: SaveContentStatus::Empty,
            revision: SaveFileRevision::absent(),
            file_size: 0,
            modified: None,
            modified_unavailable: false,
        }
    }

    fn loadable_manual_entry(slot: SaveSlotId) -> SaveCatalogEntry {
        SaveCatalogEntry {
            slot,
            role: SaveSlotRole::Manual,
            capabilities: SaveSlotCapabilities {
                can_manual_save: true,
                can_scheduler_save: false,
                can_load: true,
            },
            content: SaveContentStatus::CurrentV1 { worldgen_seed: 1 },
            revision: SaveFileRevision {
                exists: true,
                length: 1,
                modified: None,
                file_identity: Some(1),
                prefix_fingerprint: 1,
            },
            file_size: 1,
            modified: None,
            modified_unavailable: false,
        }
    }

    #[test]
    fn empty_manual_slot_issues_absent_save_for_owning_session() {
        let mut catalog = SaveCatalog::default();
        catalog.replace_entries(vec![empty_manual_entry(SaveSlotId::Manual1)], 0);
        let ui = SaveCatalogUi {
            mode: SaveCatalogMode::SaveCatalog,
            selected: None,
            session: 7,
        };
        let mut state = SaveLoadState::Idle;
        assert!(request_manual_save_for_slot(
            &mut state,
            &catalog,
            &ui,
            SaveSlotId::Manual1,
            7
        ));
        assert!(!request_manual_save_for_slot(
            &mut state,
            &catalog,
            &ui,
            SaveSlotId::Manual1,
            8
        ));
        let _ = ui;
    }

    #[test]
    fn load_confirmation_returns_to_its_catalog_owner() {
        let mut normal = SaveCatalogUi {
            mode: SaveCatalogMode::LoadCatalog,
            selected: None,
            session: 7,
        };
        assert!(begin_load_confirm(&mut normal, SaveSlotId::Manual1, 7));
        assert!(matches!(
            normal.mode,
            SaveCatalogMode::LoadConfirm {
                recovery: false,
                ..
            }
        ));
        assert!(cancel_confirm(&mut normal, 7));
        assert_eq!(normal.mode, SaveCatalogMode::LoadCatalog);

        let mut recovery = SaveCatalogUi {
            mode: SaveCatalogMode::RecoveryLoadCatalog,
            selected: None,
            session: 8,
        };
        assert!(begin_load_confirm(&mut recovery, SaveSlotId::Manual1, 8));
        assert!(matches!(
            recovery.mode,
            SaveCatalogMode::LoadConfirm { recovery: true, .. }
        ));
        assert!(cancel_confirm(&mut recovery, 8));
        assert_eq!(recovery.mode, SaveCatalogMode::RecoveryLoadCatalog);
    }

    #[test]
    fn catalog_owner_must_match_recovery_mode_before_issuing_load() {
        let mut catalog = SaveCatalog::default();
        catalog.replace_entries(vec![loadable_manual_entry(SaveSlotId::Manual1)], 0);
        let mut state = SaveLoadState::Idle;
        let normal_confirm = SaveCatalogUi {
            mode: SaveCatalogMode::LoadConfirm {
                slot: SaveSlotId::Manual1,
                recovery: false,
            },
            selected: Some(SaveSlotId::Manual1),
            session: 3,
        };
        assert!(!request_catalog_load(
            &mut state,
            &catalog,
            &normal_confirm,
            SaveRecoveryMode::RecoveryFailed,
            SaveSlotId::Manual1,
            3,
        ));
        assert!(state.is_idle());
    }
}
