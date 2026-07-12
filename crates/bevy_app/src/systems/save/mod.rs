//! セーブ/ロード機能のプラグイン。
//!
//! F5 でセーブ、F9 でロードをトリガーする（`docs/save_load.md` 参照）。
//! セーブ/ロードは同期的な exclusive system として実装されており、
//! despawn → deserialize → write → キャッシュ再構築を 1 フレーム内で完結させる
//! （plan が想定していた複数フレームにまたがる `Time<Virtual>` 一時停止パイプラインは
//! 採用していない。1フレーム内で完結させることで実装・検証を単純化した）。

mod entities;
mod load;
mod register;
mod rehydrate;
mod saving;
mod state;

use bevy::prelude::*;

pub use state::{SAVE_FILE_PATH, SaveLoadState};

use load::load_world_system;
use register::register_save_types;
use saving::save_world_system;
use state::{is_load_requested, is_save_requested, save_load_keybind_system};

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        register_save_types(app);

        app.init_resource::<SaveLoadState>();

        app.add_systems(Update, save_load_keybind_system);

        app.add_systems(Update, save_world_system.run_if(is_save_requested));
        app.add_systems(Update, load_world_system.run_if(is_load_requested));
    }
}
