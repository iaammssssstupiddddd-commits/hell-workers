//! セーブ/ロードのトリガー状態管理
//!
//! `SaveLoadState` は入力アクションまたは UI intent handler から
//! `SaveRequested` / `LoadRequested` にセットされ、`Last`のexclusive apply
//! dispatcherがこれを実行後に`Idle`へ戻す。F9 は確認ダイアログを経由し、
//! confirm 後にだけ `LoadRequested` になる。

use bevy::prelude::*;
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
