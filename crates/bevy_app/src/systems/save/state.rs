//! セーブ/ロードのトリガー状態管理
//!
//! `SaveLoadState` はキー入力（F5=セーブ, F9=ロード）を検出した通常システムから
//! `SaveRequested` / `LoadRequested` にセットされ、`save_world_system` /
//! `load_world_system`（exclusive system）が `run_if` でこれを検知して実行後に
//! `Idle` へ戻す。

use bevy::prelude::*;
use hw_ui::components::UiInputState;

/// セーブファイルの保存先（ワークスペースルートからの相対パス）
pub const SAVE_FILE_PATH: &str = "saves/world.scn.ron";

/// セーブ時点の worldgen seed。
///
/// 地形チャンク等のビジュアルは起動時に `GeneratedWorldLayoutResource` の seed から
/// 生成され、セーブには含まれない。別 seed のセッションにロードすると論理
/// （`WorldMap`）と地形表示が食い違うため、ロード時にこの値を照合して
/// 不一致ならロードを中止する（`load.rs` の seed ガード参照）。
#[derive(Resource, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Resource)]
pub struct SavedWorldgenSeed(pub u64);

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLoadState {
    #[default]
    Idle,
    SaveRequested,
    LoadRequested,
}

/// F5 でセーブ要求、F9 でロード要求を `SaveLoadState` にセットする。
/// 既にセーブ/ロードが要求中の場合は多重リクエストを無視する。
pub fn save_load_keybind_system(
    buttons: Res<ButtonInput<KeyCode>>,
    ui_input_state: Res<UiInputState>,
    mut state: ResMut<SaveLoadState>,
) {
    if hw_ui::interaction::text_input_blocks_keybinds(&ui_input_state) {
        return;
    }
    if *state != SaveLoadState::Idle {
        return;
    }
    if buttons.just_pressed(KeyCode::F5) {
        *state = SaveLoadState::SaveRequested;
        info!("Save requested (F5)");
    } else if buttons.just_pressed(KeyCode::F9) {
        *state = SaveLoadState::LoadRequested;
        info!("Load requested (F9)");
    }
}

pub fn is_save_requested(state: Res<SaveLoadState>) -> bool {
    *state == SaveLoadState::SaveRequested
}

pub fn is_load_requested(state: Res<SaveLoadState>) -> bool {
    *state == SaveLoadState::LoadRequested
}
