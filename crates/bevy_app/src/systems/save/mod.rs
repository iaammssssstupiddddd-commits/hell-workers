//! セーブ/ロード機能のプラグイン。
//!
//! F5 でセーブ、F9 でロードをトリガーする（`docs/save_load.md` 参照）。
//! セーブ/ロードは同期的な exclusive system として実装されており、
//! despawn → deserialize → write → キャッシュ再構築を 1 フレーム内で完結させる
//! （plan が想定していた複数フレームにまたがる `Time<Virtual>` 一時停止パイプラインは
//! 採用していない。1フレーム内で完結させることで実装・検証を単純化した）。

mod format;
mod load;
mod rehydrate;
mod reset;
mod saving;
mod schema;
mod state;
mod transaction;

use bevy::prelude::*;

use crate::systems::settings::SettingsPersistenceSet;

pub use state::{
    SAVE_FILE_PATH, SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult,
    SaveLoadState, SavePath,
};

use load::load_world_system;
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
}

fn save_load_apply_system(world: &mut World) {
    save_load_apply_with(world, save_world_system, load_world_system);
}

fn save_load_apply_with(
    world: &mut World,
    mut save: impl FnMut(&mut World) -> SaveLoadResult,
    mut load: impl FnMut(&mut World) -> SaveLoadResult,
) {
    let request = *world.resource::<SaveLoadState>();
    let operation = match request {
        SaveLoadState::Idle => return,
        SaveLoadState::SaveRequested => SaveLoadOperation::Save,
        SaveLoadState::LoadRequested => SaveLoadOperation::Load,
    };

    // Clear the trigger before entering fallible work so failures cannot block
    // later requests. The terminal outcome is emitted only after all load
    // resets and rollback work have completed.
    *world.resource_mut::<SaveLoadState>() = SaveLoadState::Idle;
    let target = state::save_target_label(world.resource::<SavePath>().as_path());
    let result = match operation {
        SaveLoadOperation::Save => save(world),
        SaveLoadOperation::Load => load(world),
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
