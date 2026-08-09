//! セーブ/ロード機能のプラグイン。
//!
//! F5 / F9 は catalog modal を開き、operation と slot を束ねた one-shot request を
//! `Last::SaveLoadApplySet` で適用する（`docs/save_load.md` 参照）。

mod atomic_file;
pub(crate) mod autosave;
pub(crate) mod catalog;
mod catalog_present;
pub(crate) mod catalog_ui;
mod format;
mod load;
mod metrics;
mod native_acceptance;
mod rehydrate;
mod reset;
mod saving;
mod schema;
mod state;
mod transaction;

use bevy::prelude::*;
use hw_core::SaveSlotId;

use crate::systems::settings::SettingsPersistenceSet;

pub use autosave::AutosaveScheduler;
pub use catalog::{
    AutosaveGenerationLimit, DEFAULT_SAVE_STORAGE_ROOT, SaveCatalog, SaveCatalogEntry,
    SaveContentStatus, SaveExpectedTarget, SaveFileRevision, SaveSlotCapabilities, SaveStorageRoot,
    format_relative_modified, read_save_file_revision, refresh_save_catalog,
    relative_modified_label, scan_save_catalog, slot_capabilities,
};
pub use catalog_ui::{SaveCatalogMode, SaveCatalogUi};
pub use metrics::{SaveTransactionMetrics, SaveTransactionSample};
pub use native_acceptance::NativeSaveLoadAcceptancePlugin;
pub use saving::expected_target_from_revision;
pub use state::{
    LoadRequestOrigin, SAVE_FILE_PATH, SaveDialogSession, SaveLoadFailureKind, SaveLoadOperation,
    SaveLoadOutcome, SaveLoadOutcomeSource, SaveLoadRequest, SaveLoadResult, SaveLoadState,
    SavePath, SaveRecoveryMode, SaveRequestOrigin,
};

use load::{load_world_system, recover_world_system};
#[cfg(test)]
pub(crate) use rehydrate::resolved_rehydrate_plan_names;
pub(crate) use rehydrate::{register_logic_rehydrate_pipeline, register_visual_rehydrate_pipeline};
pub(crate) use reset::{
    register_load_reset_hook, reset_root_interaction_state, reset_runtime_caches,
};
use saving::save_world_system;
use schema::register_save_types;

/// The sole project-owned final phase that may write or replace the persisted
/// world. Input and UI systems only write `SaveLoadState` during `Update`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SaveLoadApplySet;

/// Request currently being executed by the Last apply dispatcher.
#[derive(Resource, Debug, Clone, Default)]
pub(crate) struct DispatchedSaveLoadRequest(pub Option<SaveLoadRequest>);

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        register_save_types(app);
        app.init_resource::<SaveLoadState>();
        app.init_resource::<SaveRecoveryMode>();
        app.init_resource::<SavePath>();
        app.init_resource::<SaveStorageRoot>();
        app.init_resource::<SaveCatalog>();
        app.init_resource::<SaveDialogSession>();
        app.init_resource::<SaveCatalogUi>();
        app.init_resource::<AutosaveScheduler>();
        app.init_resource::<SaveTransactionMetrics>();
        app.init_resource::<DispatchedSaveLoadRequest>();
        app.init_resource::<hw_core::WorldEpoch>();
        app.add_message::<SaveLoadOutcome>();

        register_load_reset_hook(app, "root-interaction", reset_root_interaction_state);
        register_load_reset_hook(app, "root-runtime-caches", reset_runtime_caches);
        register_load_reset_hook(
            app,
            "save-catalog-interaction",
            reset_save_catalog_interaction,
        );
        register_load_reset_hook(app, "save-load-outcomes", clear_save_load_outcomes);

        app.configure_sets(Last, SaveLoadApplySet.after(SettingsPersistenceSet));
        app.add_systems(Last, save_load_apply_system.in_set(SaveLoadApplySet));
    }

    fn finish(&self, app: &mut App) {
        rehydrate::freeze_rehydrate_pipeline(app);
    }
}

/// Runtime systems that depend on settings/UI assets. Registered by the app shell.
pub fn register_save_catalog_runtime_systems(app: &mut App) {
    use crate::interface::ui::interaction::handle_ui_intent;
    use crate::systems::GameSystemSet;

    app.add_systems(
        Update,
        (
            catalog_ui::settle_catalog_after_outcomes_system,
            catalog::refresh_dirty_save_catalog_system,
            catalog_present::sync_save_catalog_dialog_system,
            autosave::tick_autosave_timer_system,
        )
            .chain()
            .in_set(GameSystemSet::Interface)
            .before(handle_ui_intent),
    );
    app.add_systems(
        Update,
        (
            autosave::consume_autosave_timer_on_outcomes,
            autosave::autosave_scheduler_system,
        )
            .chain()
            .after(handle_ui_intent)
            .in_set(GameSystemSet::Interface),
    );
}

pub(crate) fn save_load_apply_system(world: &mut World) {
    save_load_apply_with(
        world,
        save_world_system,
        load_world_system,
        recover_world_system,
    );
}

fn save_load_apply_with(
    world: &mut World,
    mut save: impl FnMut(&mut World) -> SaveLoadResult,
    mut load: impl FnMut(&mut World) -> SaveLoadResult,
    mut recover: impl FnMut(&mut World) -> SaveLoadResult,
) {
    let Some(request) = world.resource_mut::<SaveLoadState>().take_request() else {
        return;
    };
    let resolved = world.resource::<SaveStorageRoot>().resolve(request.slot());
    *world.resource_mut::<SavePath>() = SavePath::new(resolved);
    world.resource_mut::<DispatchedSaveLoadRequest>().0 = Some(request.clone());

    let recovery_required = world
        .get_resource::<SaveRecoveryMode>()
        .is_some_and(|mode| *mode == SaveRecoveryMode::RecoveryFailed);
    let result = match &request {
        SaveLoadRequest::Save { .. } if recovery_required => {
            SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed)
        }
        SaveLoadRequest::Load {
            origin: LoadRequestOrigin::NormalCatalog { .. },
            ..
        } if recovery_required => SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
        SaveLoadRequest::Load {
            origin: LoadRequestOrigin::RecoveryCatalog { .. },
            ..
        } if !recovery_required => SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
        SaveLoadRequest::Save { .. } => save(world),
        SaveLoadRequest::Load {
            origin: LoadRequestOrigin::NormalCatalog { .. },
            ..
        } => load(world),
        SaveLoadRequest::Load {
            origin: LoadRequestOrigin::RecoveryCatalog { .. },
            ..
        } => recover(world),
    };
    world.resource_mut::<DispatchedSaveLoadRequest>().0 = None;
    if let Some(mut catalog) = world.get_resource_mut::<SaveCatalog>() {
        match (&request, result) {
            (
                SaveLoadRequest::Load { slot, .. },
                SaveLoadResult::Failed(SaveLoadFailureKind::InvalidData),
            ) => catalog.mark_body_invalid(*slot),
            (
                SaveLoadRequest::Save { slot, .. },
                SaveLoadResult::Succeeded
                | SaveLoadResult::Failed(SaveLoadFailureKind::CommittedDurabilityUncertain),
            ) => {
                catalog.clear_body_invalid(*slot);
                catalog.mark_dirty();
            }
            _ => catalog.mark_dirty(),
        }
    }
    world.write_message(SaveLoadOutcome::from_request(&request, result));
}

fn clear_save_load_outcomes(world: &mut World) {
    if let Some(mut outcomes) = world.get_resource_mut::<Messages<SaveLoadOutcome>>() {
        outcomes.clear();
    }
}

/// A world replacement invalidates the modal's selection/confirmation state,
/// but not the monotonically increasing session counter. Keeping the counter
/// prevents a stale button payload from becoming valid after a reload.
fn reset_save_catalog_interaction(world: &mut World) {
    if let Some(mut catalog_ui) = world.get_resource_mut::<SaveCatalogUi>() {
        catalog_ui.mode = SaveCatalogMode::Closed;
        catalog_ui.selected = None;
    }
    if let Some(mut catalog) = world.get_resource_mut::<SaveCatalog>() {
        catalog.mark_dirty();
    }
}

/// Test / driver helper: manual save against an absent target.
pub fn manual_save_request(slot: SaveSlotId, dialog_session: u64) -> SaveLoadRequest {
    SaveLoadRequest::Save {
        origin: SaveRequestOrigin::ManualCatalog { dialog_session },
        slot,
        expected_target: SaveExpectedTarget::Absent,
    }
}

/// Test / driver helper: normal catalog load.
pub fn normal_load_request(slot: SaveSlotId, dialog_session: u64) -> SaveLoadRequest {
    SaveLoadRequest::Load {
        origin: LoadRequestOrigin::NormalCatalog { dialog_session },
        slot,
    }
}

/// Recovery-only load. Production UI must obtain this through the catalog owner.
pub fn recovery_load_request(slot: SaveSlotId, dialog_session: u64) -> SaveLoadRequest {
    SaveLoadRequest::Load {
        origin: LoadRequestOrigin::RecoveryCatalog { dialog_session },
        slot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;
    use hw_ui::HwUiPlugin;
    use hw_ui::notifications::{
        NotificationCenter, NotificationRetention, NotificationSeverity, UserFacingNotification,
        reduce_notifications_system,
    };
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_outcome(
        operation: SaveLoadOperation,
        target: &str,
        result: SaveLoadResult,
        source: SaveLoadOutcomeSource,
    ) -> SaveLoadOutcome {
        SaveLoadOutcome {
            operation,
            target: target.to_owned(),
            result,
            source,
        }
    }

    fn manual_source() -> SaveLoadOutcomeSource {
        SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog { dialog_session: 1 })
    }

    fn normal_load_source() -> SaveLoadOutcomeSource {
        SaveLoadOutcomeSource::Load(LoadRequestOrigin::NormalCatalog { dialog_session: 1 })
    }

    fn recovery_load_source() -> SaveLoadOutcomeSource {
        SaveLoadOutcomeSource::Load(LoadRequestOrigin::RecoveryCatalog { dialog_session: 1 })
    }

    fn request_load(mut state: ResMut<SaveLoadState>) {
        let _ = state.try_set(normal_load_request(SaveSlotId::Manual1, 1));
    }

    fn dispatch_world() -> World {
        let mut world = World::new();
        world.init_resource::<SaveLoadState>();
        world.init_resource::<SavePath>();
        world.init_resource::<SaveStorageRoot>();
        world.init_resource::<SaveCatalog>();
        world.init_resource::<DispatchedSaveLoadRequest>();
        world.init_resource::<Messages<SaveLoadOutcome>>();
        world
    }

    #[test]
    fn update_request_is_consumed_once_by_the_last_apply_phase() {
        let mut app = minimal_app();
        app.add_plugins(SavePlugin);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "hell-workers-missing-load-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        app.insert_resource(SaveStorageRoot::new(root.clone()));
        app.add_systems(Update, request_load);

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::Idle
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<SaveLoadOutcome>>()
                .drain()
                .collect::<Vec<_>>(),
            vec![test_outcome(
                SaveLoadOperation::Load,
                "Manual slot 1",
                SaveLoadResult::Failed(SaveLoadFailureKind::LoadNotFound),
                normal_load_source(),
            )]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dispatcher_clears_request_before_work_and_emits_exactly_one_outcome() {
        let mut world = dispatch_world();
        assert!(
            world
                .resource_mut::<SaveLoadState>()
                .try_set(manual_save_request(SaveSlotId::Manual1, 1))
        );
        let calls = Cell::new(0);

        save_load_apply_with(
            &mut world,
            |world| {
                calls.set(calls.get() + 1);
                assert_eq!(*world.resource::<SaveLoadState>(), SaveLoadState::Idle);
                SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite)
            },
            |_| panic!("load executor must not run"),
            |_| panic!("recovery executor must not run"),
        );

        assert_eq!(calls.get(), 1);
        assert_eq!(
            world
                .resource_mut::<Messages<SaveLoadOutcome>>()
                .drain()
                .collect::<Vec<_>>(),
            vec![test_outcome(
                SaveLoadOperation::Save,
                "Manual slot 1",
                SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite),
                manual_source(),
            )]
        );
    }

    #[test]
    fn dispatcher_emits_one_outcome_for_every_save_terminal_result() {
        let results = [
            SaveLoadResult::Succeeded,
            SaveLoadResult::Failed(SaveLoadFailureKind::SaveSerialize),
            SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite),
        ];

        for result in results {
            let mut world = dispatch_world();
            assert!(
                world
                    .resource_mut::<SaveLoadState>()
                    .try_set(manual_save_request(SaveSlotId::Manual1, 1))
            );

            save_load_apply_with(
                &mut world,
                |_| result,
                |_| panic!("load executor must not run"),
                |_| panic!("recovery executor must not run"),
            );

            assert_eq!(
                world
                    .resource_mut::<Messages<SaveLoadOutcome>>()
                    .drain()
                    .collect::<Vec<_>>(),
                vec![test_outcome(
                    SaveLoadOperation::Save,
                    "Manual slot 1",
                    result,
                    manual_source(),
                )]
            );
        }
    }

    #[test]
    fn dispatcher_rejects_save_while_recovery_is_required() {
        let mut world = dispatch_world();
        world.insert_resource(SaveRecoveryMode::RecoveryFailed);
        assert!(
            world
                .resource_mut::<SaveLoadState>()
                .try_set(manual_save_request(SaveSlotId::Manual1, 1))
        );

        save_load_apply_with(
            &mut world,
            |_| panic!("save executor must stay disabled in recovery mode"),
            |_| panic!("load executor must not run for a save request"),
            |_| panic!("recovery executor must not run for a save request"),
        );

        assert_eq!(
            world
                .resource_mut::<Messages<SaveLoadOutcome>>()
                .drain()
                .collect::<Vec<_>>(),
            vec![test_outcome(
                SaveLoadOperation::Save,
                "Manual slot 1",
                SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
                manual_source(),
            )]
        );
    }

    #[test]
    fn dispatcher_rejects_normal_load_while_recovery_is_required() {
        let mut world = dispatch_world();
        world.insert_resource(SaveRecoveryMode::RecoveryFailed);
        assert!(
            world
                .resource_mut::<SaveLoadState>()
                .try_set(normal_load_request(SaveSlotId::Manual1, 1))
        );

        save_load_apply_with(
            &mut world,
            |_| panic!("save executor must not run for a load request"),
            |_| panic!("normal load executor must stay disabled in recovery mode"),
            |_| panic!("recovery executor requires the dedicated trigger"),
        );

        assert_eq!(*world.resource::<SaveLoadState>(), SaveLoadState::Idle);
        assert_eq!(
            world
                .resource_mut::<Messages<SaveLoadOutcome>>()
                .drain()
                .collect::<Vec<_>>(),
            vec![test_outcome(
                SaveLoadOperation::Load,
                "Manual slot 1",
                SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
                normal_load_source(),
            )]
        );
    }

    #[test]
    fn dispatcher_allows_only_the_dedicated_recovery_trigger_in_recovery_mode() {
        for (mode, should_recover) in [
            (SaveRecoveryMode::Healthy, false),
            (SaveRecoveryMode::RecoveryFailed, true),
        ] {
            let mut world = dispatch_world();
            world.insert_resource(mode);
            assert!(
                world
                    .resource_mut::<SaveLoadState>()
                    .try_set(recovery_load_request(SaveSlotId::Manual1, 1))
            );
            let recover_calls = Cell::new(0);

            save_load_apply_with(
                &mut world,
                |_| panic!("save executor must not run for a recovery request"),
                |_| panic!("normal load executor must not run for a recovery request"),
                |_| {
                    recover_calls.set(recover_calls.get() + 1);
                    SaveLoadResult::Succeeded
                },
            );

            assert_eq!(recover_calls.get(), usize::from(should_recover));
            assert_eq!(*world.resource::<SaveLoadState>(), SaveLoadState::Idle);
            let expected_result = if should_recover {
                SaveLoadResult::Succeeded
            } else {
                SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed)
            };
            assert_eq!(
                world
                    .resource_mut::<Messages<SaveLoadOutcome>>()
                    .drain()
                    .collect::<Vec<_>>(),
                vec![test_outcome(
                    SaveLoadOperation::Load,
                    "Manual slot 1",
                    expected_result,
                    recovery_load_source(),
                )]
            );
        }
    }

    #[test]
    fn load_outcome_is_written_after_executor_resets_messages() {
        let mut world = dispatch_world();
        assert!(
            world
                .resource_mut::<SaveLoadState>()
                .try_set(normal_load_request(SaveSlotId::Manual1, 1))
        );
        world.write_message(test_outcome(
            SaveLoadOperation::Save,
            "old",
            SaveLoadResult::Succeeded,
            manual_source(),
        ));

        save_load_apply_with(
            &mut world,
            |_| panic!("save executor must not run"),
            |world| {
                clear_save_load_outcomes(world);
                SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered)
            },
            |_| panic!("recovery executor must not run"),
        );

        assert_eq!(
            world
                .resource_mut::<Messages<SaveLoadOutcome>>()
                .drain()
                .collect::<Vec<_>>(),
            vec![test_outcome(
                SaveLoadOperation::Load,
                "Manual slot 1",
                SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered),
                normal_load_source(),
            )]
        );
    }

    #[test]
    fn terminal_load_outcomes_become_the_first_history_entry_after_ui_reset() {
        let cases = [
            (SaveLoadResult::Succeeded, "Game loaded"),
            (
                SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered),
                "Load failed; world restored",
            ),
            (
                SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
                "Load recovery failed",
            ),
        ];

        for (result, expected_title) in cases {
            let mut app = minimal_app();
            app.add_plugins(HwUiPlugin)
                .add_message::<SaveLoadOutcome>()
                .init_resource::<SaveLoadState>()
                .init_resource::<SavePath>()
                .init_resource::<SaveStorageRoot>()
                .init_resource::<SaveCatalog>()
                .init_resource::<DispatchedSaveLoadRequest>()
                .add_systems(
                    Update,
                    (
                        crate::interface::ui::notifications::adapt_save_load_outcomes,
                        reduce_notifications_system,
                    )
                        .chain(),
                );
            assert!(
                app.world_mut()
                    .resource_mut::<SaveLoadState>()
                    .try_set(normal_load_request(SaveSlotId::Manual1, 1))
            );
            app.world_mut().resource_mut::<NotificationCenter>().push(
                UserFacingNotification::new(
                    "old-world",
                    NotificationSeverity::Warning,
                    "Old world entry",
                    "stale",
                    NotificationRetention::Important,
                ),
                std::time::Duration::ZERO,
            );

            save_load_apply_with(
                app.world_mut(),
                |_| panic!("save executor must not run"),
                |world| {
                    clear_save_load_outcomes(world);
                    hw_ui::reset_for_world_replace(world);
                    result
                },
                |_| panic!("recovery executor must not run"),
            );
            app.update();

            let center = app.world().resource::<NotificationCenter>();
            assert_eq!(center.history_count(), 1);
            assert_eq!(
                center.history_entries().next().unwrap().title,
                expected_title
            );
        }
    }
}
