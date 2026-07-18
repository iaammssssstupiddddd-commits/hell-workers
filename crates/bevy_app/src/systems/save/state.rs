//! セーブ/ロードのトリガー状態管理
//!
//! `SaveLoadState` は入力アクションまたは UI intent handler から
//! `SaveRequested` / `LoadRequested` にセットされ、`Last`のexclusive apply
//! dispatcherが処理前に`Idle`へ戻す。F9 は対象が存在する場合は確認後、
//! 存在しない場合はowner側のread結果を得るため確認なしで`LoadRequested`になる。

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

/// A terminal save/load operation. This remains separate from
/// [`SaveLoadState`], which is only a one-shot dispatcher trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveLoadOperation {
    Save,
    Load,
}

impl SaveLoadOperation {
    pub(crate) const fn key_part(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Load => "load",
        }
    }
}

/// Display-safe failure categories. Detailed OS, serialization, and
/// transaction errors stay in logs and never cross the UI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveLoadFailureKind {
    SaveSerialize,
    SaveWrite,
    LoadNotFound,
    LoadRead,
    UnsupportedFormat,
    InvalidData,
    SeedMismatch,
    MissingPrerequisite,
    ApplyRecovered,
    RecoveryFailed,
}

impl SaveLoadFailureKind {
    pub(crate) const fn key_part(self) -> &'static str {
        match self {
            Self::SaveSerialize => "save_serialize",
            Self::SaveWrite => "save_write",
            Self::LoadNotFound => "load_not_found",
            Self::LoadRead => "load_read",
            Self::UnsupportedFormat => "unsupported_format",
            Self::InvalidData => "invalid_data",
            Self::SeedMismatch => "seed_mismatch",
            Self::MissingPrerequisite => "missing_prerequisite",
            Self::ApplyRecovered => "apply_recovered",
            Self::RecoveryFailed => "recovery_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveLoadResult {
    Succeeded,
    Failed(SaveLoadFailureKind),
}

impl SaveLoadResult {
    pub(crate) const fn key_part(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed(failure) => failure.key_part(),
        }
    }
}

/// The one terminal result emitted for each consumed save/load request.
/// `target` is a display-safe label, never an absolute path.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct SaveLoadOutcome {
    pub operation: SaveLoadOperation,
    pub target: String,
    pub result: SaveLoadResult,
}

pub(super) fn save_target_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.chars().any(char::is_control))
        .unwrap_or("Current save")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_label_exposes_only_a_safe_file_name() {
        assert_eq!(
            save_target_label(Path::new("/private/session/world.scn.ron")),
            "world.scn.ron"
        );
        assert_eq!(save_target_label(Path::new("/")), "Current save");
        assert_eq!(
            save_target_label(Path::new("saves/unsafe\nname.ron")),
            "Current save"
        );
    }
}
