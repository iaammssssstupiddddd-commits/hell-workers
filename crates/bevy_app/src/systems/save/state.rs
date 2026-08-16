//! セーブ/ロードのトリガー状態管理
//!
//! Input / UI は operation・slot・revision・dialog session を束ねた
//! [`SaveLoadRequest`] を one-shot pending として書き込む。`Last` の exclusive
//! apply dispatcher が処理前に `Idle` へ戻す。
//! `RecoveryCatalog` origin は rollback 失敗後の foreground recovery owner だけが
//! 発行でき、通常の F9 / raw UiIntent からは構築できない。

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use hw_core::SaveSlotId;

use super::catalog::SaveExpectedTarget;

/// 互換用の旧単一セーブパス定数。legacy default slot の canonical 名と一致する。
pub const SAVE_FILE_PATH: &str = "saves/world.scn.ron";

/// 過渡互換の単一 path。正本は `SaveStorageRoot + SaveSlotId`。
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

/// Monotonic dialog session owned by the catalog modal.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SaveDialogSession(pub u64);

impl SaveDialogSession {
    pub fn bump(&mut self) -> u64 {
        self.0 = self.0.saturating_add(1);
        self.0
    }

    pub fn current(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveRequestOrigin {
    ManualCatalog { dialog_session: u64 },
    Autosave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadRequestOrigin {
    NormalCatalog {
        dialog_session: u64,
    },
    /// Capability-only origin. Must not be constructible from raw UiIntent payloads.
    RecoveryCatalog {
        dialog_session: u64,
    },
}

impl LoadRequestOrigin {
    pub const fn is_recovery(self) -> bool {
        matches!(self, Self::RecoveryCatalog { .. })
    }

    pub const fn dialog_session(self) -> u64 {
        match self {
            Self::NormalCatalog { dialog_session } | Self::RecoveryCatalog { dialog_session } => {
                dialog_session
            }
        }
    }
}

impl SaveRequestOrigin {
    pub const fn dialog_session(self) -> Option<u64> {
        match self {
            Self::ManualCatalog { dialog_session } => Some(dialog_session),
            Self::Autosave => None,
        }
    }
}

/// Immutable one-shot request consumed by `Last::SaveLoadApplySet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveLoadRequest {
    Save {
        origin: SaveRequestOrigin,
        slot: SaveSlotId,
        expected_target: SaveExpectedTarget,
    },
    Load {
        origin: LoadRequestOrigin,
        slot: SaveSlotId,
    },
}

impl SaveLoadRequest {
    pub const fn operation(&self) -> SaveLoadOperation {
        match self {
            Self::Save { .. } => SaveLoadOperation::Save,
            Self::Load { .. } => SaveLoadOperation::Load,
        }
    }

    pub const fn slot(&self) -> SaveSlotId {
        match self {
            Self::Save { slot, .. } | Self::Load { slot, .. } => *slot,
        }
    }

    pub fn target_label(&self) -> String {
        self.slot().player_label().to_owned()
    }
}

#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub enum SaveLoadState {
    #[default]
    Idle,
    Pending(SaveLoadRequest),
}

impl SaveLoadState {
    pub fn take_request(&mut self) -> Option<SaveLoadRequest> {
        match std::mem::take(self) {
            Self::Pending(request) => Some(request),
            Self::Idle => None,
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Accepts a request only when idle. Returns false without overwriting.
    pub fn try_set(&mut self, request: SaveLoadRequest) -> bool {
        if !self.is_idle() {
            return false;
        }
        *self = Self::Pending(request);
        true
    }
}

/// Coordinator-owned trust state for the live simulation world.
///
/// Only a rollback failure enters `RecoveryFailed`. Normal save/load requests
/// remain disabled until a fully preflighted recovery-only replacement succeeds.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRecoveryMode {
    #[default]
    Healthy,
    RecoveryFailed,
}

/// Opt-in fault arm used exclusively by the bounded native save-catalog
/// acceptance driver. It is never initialized by the production plugin,
/// never reflected, and never included in a world snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum NativeLoadFault {
    #[default]
    None,
    /// Fail after a normal live write so the rollback snapshot succeeds.
    ApplyRecovered,
    /// Fail after a normal live write, then fail rollback finalization.
    RecoveryFailedNormalApply,
    /// Internal second phase of `RecoveryFailedNormalApply`.
    RecoveryFailedRollbackFinalize,
    /// Fail an already-recovery-only replacement after it has written live entities.
    RecoveryOnlyApply,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct NativeLoadFaultInjection(pub NativeLoadFault);

impl NativeLoadFaultInjection {
    pub(crate) fn arm(&mut self, fault: NativeLoadFault) {
        debug_assert!(matches!(self.0, NativeLoadFault::None));
        self.0 = fault;
    }

    pub(crate) fn fail_normal_post_write(&mut self) -> Option<&'static str> {
        match self.0 {
            NativeLoadFault::ApplyRecovered => {
                self.0 = NativeLoadFault::None;
                Some("native acceptance injected normal apply failure")
            }
            NativeLoadFault::RecoveryFailedNormalApply => {
                self.0 = NativeLoadFault::RecoveryFailedRollbackFinalize;
                Some("native acceptance injected normal apply failure before rollback")
            }
            _ => None,
        }
    }

    pub(crate) fn fail_rollback_finalize(&mut self) -> Option<&'static str> {
        if self.0 == NativeLoadFault::RecoveryFailedRollbackFinalize {
            self.0 = NativeLoadFault::None;
            Some("native acceptance injected rollback finalization failure")
        } else {
            None
        }
    }

    pub(crate) fn fail_recovery_only_post_write(&mut self) -> Option<&'static str> {
        if self.0 == NativeLoadFault::RecoveryOnlyApply {
            self.0 = NativeLoadFault::None;
            Some("native acceptance injected recovery-only apply failure")
        } else {
            None
        }
    }
}

/// Job-private fault arm for the frozen RtT-light P05 behavior harness.
///
/// The resource exists only in profiling builds, is inserted only by the
/// selected behavior case, and is neither reflected nor persisted.
#[cfg(feature = "profiling")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PerfLoadFault {
    #[default]
    None,
    ApplyRecovered,
    RecoveryFailedNormalApply,
    RecoveryFailedRollbackFinalize,
    DuplicateReset,
}

#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PerfLoadFaultInjection(pub PerfLoadFault);

#[cfg(feature = "profiling")]
impl PerfLoadFaultInjection {
    pub(crate) fn arm(&mut self, fault: PerfLoadFault) {
        debug_assert!(matches!(self.0, PerfLoadFault::None));
        self.0 = fault;
    }

    pub(crate) fn fail_normal_post_write(&mut self) -> Option<&'static str> {
        match self.0 {
            PerfLoadFault::ApplyRecovered => {
                self.0 = PerfLoadFault::None;
                Some("P05 behavior injected normal apply failure")
            }
            PerfLoadFault::RecoveryFailedNormalApply => {
                self.0 = PerfLoadFault::RecoveryFailedRollbackFinalize;
                Some("P05 behavior injected normal apply failure before rollback")
            }
            _ => None,
        }
    }

    pub(crate) fn fail_rollback_finalize(&mut self) -> Option<&'static str> {
        if self.0 == PerfLoadFault::RecoveryFailedRollbackFinalize {
            self.0 = PerfLoadFault::None;
            Some("P05 behavior injected rollback finalization failure")
        } else {
            None
        }
    }

    pub(crate) fn take_duplicate_reset(&mut self) -> bool {
        if self.0 == PerfLoadFault::DuplicateReset {
            self.0 = PerfLoadFault::None;
            true
        } else {
            false
        }
    }
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
    OverwriteConfirmationRequired,
    CommittedDurabilityUncertain,
    RequestRejected,
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
            Self::OverwriteConfirmationRequired => "overwrite_confirmation_required",
            Self::CommittedDurabilityUncertain => "committed_durability_uncertain",
            Self::RequestRejected => "request_rejected",
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

/// Who produced the terminal save/load request behind this outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveLoadOutcomeSource {
    Manual(SaveRequestOrigin),
    Autosave,
    Load(LoadRequestOrigin),
}

impl SaveLoadOutcomeSource {
    pub fn from_request(request: &SaveLoadRequest) -> Self {
        match request {
            SaveLoadRequest::Save { origin, .. } => match origin {
                SaveRequestOrigin::Autosave => Self::Autosave,
                SaveRequestOrigin::ManualCatalog { .. } => Self::Manual(*origin),
            },
            SaveLoadRequest::Load { origin, .. } => Self::Load(*origin),
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
    pub source: SaveLoadOutcomeSource,
}

impl SaveLoadOutcome {
    pub fn from_request(request: &SaveLoadRequest, result: SaveLoadResult) -> Self {
        Self {
            operation: request.operation(),
            target: request.target_label().to_owned(),
            result,
            source: SaveLoadOutcomeSource::from_request(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::catalog::SaveFileRevision;

    #[test]
    fn pending_request_keeps_slot_and_revision_immutable() {
        let request = SaveLoadRequest::Save {
            origin: SaveRequestOrigin::ManualCatalog { dialog_session: 3 },
            slot: SaveSlotId::Manual2,
            expected_target: SaveExpectedTarget::Exact(SaveFileRevision::absent()),
        };
        let mut state = SaveLoadState::Idle;
        assert!(state.try_set(request.clone()));
        assert!(!state.try_set(SaveLoadRequest::Load {
            origin: LoadRequestOrigin::NormalCatalog { dialog_session: 4 },
            slot: SaveSlotId::Manual1,
        }));
        let taken = state.take_request().unwrap();
        assert_eq!(taken.slot(), SaveSlotId::Manual2);
        assert_eq!(taken.target_label(), "Manual slot 2");
        assert!(state.is_idle());
    }
}
