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

pub use state::{SAVE_FILE_PATH, SaveLoadState, SavePath};

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

        // These root-owned hooks are registered here because they have no leaf
        // owner. Leaf facades register their own hooks when their plugins are
        // constructed, without depending on this module.
        register_load_reset_hook(app, "root-interaction", reset_root_interaction_state);
        register_load_reset_hook(app, "root-runtime-caches", reset_runtime_caches);

        app.configure_sets(Last, SaveLoadApplySet.after(SettingsPersistenceSet));
        app.add_systems(Last, save_load_apply_system.in_set(SaveLoadApplySet));
    }
}

fn save_load_apply_system(world: &mut World) {
    match *world.resource::<SaveLoadState>() {
        SaveLoadState::Idle => {}
        SaveLoadState::SaveRequested => save_world_system(world),
        SaveLoadState::LoadRequested => load_world_system(world),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;

    fn request_load(mut state: ResMut<SaveLoadState>) {
        *state = SaveLoadState::LoadRequested;
    }

    #[test]
    fn update_request_is_consumed_once_by_the_last_apply_phase() {
        let mut app = minimal_app();
        app.add_plugins(SavePlugin);
        app.insert_resource(SavePath::new(
            std::env::temp_dir().join("hell-workers-missing-load-test.ron"),
        ));
        app.add_systems(Update, request_load);

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::Idle
        );
    }
}
