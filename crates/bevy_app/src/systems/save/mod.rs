//! セーブ/ロード機能のプラグイン。
//!
//! F5 でセーブ、F9 でロードをトリガーする（`docs/save_load.md` 参照）。
//! セーブ/ロードは同期的な exclusive system として実装されており、
//! despawn → deserialize → write → キャッシュ再構築を 1 フレーム内で完結させる
//! （plan が想定していた複数フレームにまたがる `Time<Virtual>` 一時停止パイプラインは
//! 採用していない。1フレーム内で完結させることで実装・検証を単純化した）。

mod format;
mod load;
mod native_acceptance;
mod rehydrate;
mod reset;
mod saving;
mod schema;
mod state;
mod transaction;

use bevy::prelude::*;

use crate::systems::settings::SettingsPersistenceSet;

pub use native_acceptance::NativeSaveLoadAcceptancePlugin;
pub use state::{
    SAVE_FILE_PATH, SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult,
    SaveLoadState, SavePath, SaveRecoveryMode,
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

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        register_save_types(app);
        app.init_resource::<SaveLoadState>();
        app.init_resource::<SaveRecoveryMode>();
        app.init_resource::<SavePath>();
        app.init_resource::<hw_core::WorldEpoch>();
        app.add_message::<SaveLoadOutcome>();

        // These root-owned hooks are registered here because they have no leaf
        // owner. Leaf facades register their own hooks when their plugins are
        // constructed, without depending on this module.
        register_load_reset_hook(app, "root-interaction", reset_root_interaction_state);
        register_load_reset_hook(app, "root-runtime-caches", reset_runtime_caches);
        register_load_reset_hook(app, "save-load-outcomes", clear_save_load_outcomes);

        app.configure_sets(Last, SaveLoadApplySet.after(SettingsPersistenceSet));
        app.add_systems(Last, save_load_apply_system.in_set(SaveLoadApplySet));
    }

    fn finish(&self, app: &mut App) {
        rehydrate::freeze_rehydrate_pipeline(app);
    }
}

fn save_load_apply_system(world: &mut World) {
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
    let request = *world.resource::<SaveLoadState>();
    let operation = match request {
        SaveLoadState::Idle => return,
        SaveLoadState::SaveRequested => SaveLoadOperation::Save,
        SaveLoadState::LoadRequested | SaveLoadState::RecoveryLoadRequested => {
            SaveLoadOperation::Load
        }
    };

    // Clear the trigger before entering fallible work so failures cannot block
    // later requests. The terminal outcome is emitted only after all load
    // resets and rollback work have completed.
    *world.resource_mut::<SaveLoadState>() = SaveLoadState::Idle;
    let target = state::save_target_label(world.resource::<SavePath>().as_path());
    let recovery_required = world
        .get_resource::<SaveRecoveryMode>()
        .is_some_and(|mode| *mode == SaveRecoveryMode::RecoveryFailed);
    let result = match request {
        SaveLoadState::SaveRequested | SaveLoadState::LoadRequested if recovery_required => {
            SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed)
        }
        SaveLoadState::RecoveryLoadRequested if !recovery_required => {
            SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed)
        }
        SaveLoadState::SaveRequested => save(world),
        SaveLoadState::LoadRequested => load(world),
        SaveLoadState::RecoveryLoadRequested => recover(world),
        SaveLoadState::Idle => unreachable!("Idle requests return before dispatch"),
    };
    world.write_message(SaveLoadOutcome {
        operation,
        target,
        result,
    });
}

fn clear_save_load_outcomes(world: &mut World) {
    if let Some(mut outcomes) = world.get_resource_mut::<Messages<SaveLoadOutcome>>() {
        outcomes.clear();
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

    fn request_load(mut state: ResMut<SaveLoadState>) {
        *state = SaveLoadState::LoadRequested;
    }

    #[test]
    fn update_request_is_consumed_once_by_the_last_apply_phase() {
        let mut app = minimal_app();
        app.add_plugins(SavePlugin);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        let file_name = format!(
            "hell-workers-missing-load-test-{}-{nonce}.ron",
            std::process::id()
        );
        app.insert_resource(SavePath::new(std::env::temp_dir().join(&file_name)));
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
            vec![SaveLoadOutcome {
                operation: SaveLoadOperation::Load,
                target: file_name,
                result: SaveLoadResult::Failed(SaveLoadFailureKind::LoadNotFound),
            }]
        );
    }

    #[test]
    fn dispatcher_clears_request_before_work_and_emits_exactly_one_outcome() {
        let mut world = World::new();
        world.insert_resource(SaveLoadState::SaveRequested);
        world.insert_resource(SavePath::new("private/slot-a.ron"));
        world.init_resource::<Messages<SaveLoadOutcome>>();
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
            vec![SaveLoadOutcome {
                operation: SaveLoadOperation::Save,
                target: "slot-a.ron".to_owned(),
                result: SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite),
            }]
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
            let mut world = World::new();
            world.insert_resource(SaveLoadState::SaveRequested);
            world.insert_resource(SavePath::new("slot-a.ron"));
            world.init_resource::<Messages<SaveLoadOutcome>>();

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
                vec![SaveLoadOutcome {
                    operation: SaveLoadOperation::Save,
                    target: "slot-a.ron".to_owned(),
                    result,
                }]
            );
        }
    }

    #[test]
    fn dispatcher_rejects_save_while_recovery_is_required() {
        let mut world = World::new();
        world.insert_resource(SaveLoadState::SaveRequested);
        world.insert_resource(SaveRecoveryMode::RecoveryFailed);
        world.insert_resource(SavePath::new("slot-a.ron"));
        world.init_resource::<Messages<SaveLoadOutcome>>();

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
            vec![SaveLoadOutcome {
                operation: SaveLoadOperation::Save,
                target: "slot-a.ron".to_owned(),
                result: SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
            }]
        );
    }

    #[test]
    fn dispatcher_rejects_normal_load_while_recovery_is_required() {
        let mut world = World::new();
        world.insert_resource(SaveLoadState::LoadRequested);
        world.insert_resource(SaveRecoveryMode::RecoveryFailed);
        world.insert_resource(SavePath::new("slot-a.ron"));
        world.init_resource::<Messages<SaveLoadOutcome>>();

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
            vec![SaveLoadOutcome {
                operation: SaveLoadOperation::Load,
                target: "slot-a.ron".to_owned(),
                result: SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
            }]
        );
    }

    #[test]
    fn dispatcher_allows_only_the_dedicated_recovery_trigger_in_recovery_mode() {
        for (mode, should_recover) in [
            (SaveRecoveryMode::Healthy, false),
            (SaveRecoveryMode::RecoveryFailed, true),
        ] {
            let mut world = World::new();
            world.insert_resource(SaveLoadState::RecoveryLoadRequested);
            world.insert_resource(mode);
            world.insert_resource(SavePath::new("slot-a.ron"));
            world.init_resource::<Messages<SaveLoadOutcome>>();
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
                vec![SaveLoadOutcome {
                    operation: SaveLoadOperation::Load,
                    target: "slot-a.ron".to_owned(),
                    result: expected_result,
                }]
            );
        }
    }

    #[test]
    fn load_outcome_is_written_after_executor_resets_messages() {
        let mut world = World::new();
        world.insert_resource(SaveLoadState::LoadRequested);
        world.insert_resource(SavePath::new("slot-a.ron"));
        world.init_resource::<Messages<SaveLoadOutcome>>();
        world.write_message(SaveLoadOutcome {
            operation: SaveLoadOperation::Save,
            target: "old.ron".to_owned(),
            result: SaveLoadResult::Succeeded,
        });

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
            vec![SaveLoadOutcome {
                operation: SaveLoadOperation::Load,
                target: "slot-a.ron".to_owned(),
                result: SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered),
            }]
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
                .insert_resource(SaveLoadState::LoadRequested)
                .insert_resource(SavePath::new("slot-a.ron"))
                .add_systems(
                    Update,
                    (
                        crate::interface::ui::notifications::adapt_save_load_outcomes,
                        reduce_notifications_system,
                    )
                        .chain(),
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
