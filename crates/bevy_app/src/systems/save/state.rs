//! セーブ/ロードのトリガー状態管理
//!
//! `SaveLoadState` はキー入力（F5=セーブ, F9=ロード）を検出した通常システムから
//! `SaveRequested` / `LoadRequested` にセットされ、`Last`のexclusive apply
//! dispatcherがこれを実行後に`Idle`へ戻す。

use bevy::prelude::*;
use hw_ui::components::UiInputState;
use std::path::{Path, PathBuf};

/// セーブファイルの保存先（ワークスペースルートからの相対パス）
pub const SAVE_FILE_PATH: &str = "saves/world.scn.ron";

/// セーブ先。通常は既定パスを使うが、テストと将来の slot 選択では差し替えられる。
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct SavePath(pub PathBuf);

impl SavePath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Default for SavePath {
    fn default() -> Self {
        Self::new(SAVE_FILE_PATH)
    }
}

/// header 無し v0 セーブの worldgen seed。
///
/// v1 以降は外部 header が seed を保持する。この型は magic 無しの既存セーブを
/// 読む間だけ `AppTypeRegistry` に残す。
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
