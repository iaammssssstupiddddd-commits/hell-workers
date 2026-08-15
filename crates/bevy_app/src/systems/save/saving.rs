//! ワールドのセーブ（exclusive system）。
//!
//! `schema.rs` が構成する DynamicWorld allow-listで構築し、RON body を serialize したあと
//! header/body を stream して atomic commit する。

use std::fmt;
use std::time::Instant;

use bevy::prelude::*;

use super::DispatchedSaveLoadRequest;
use super::atomic_file::{
    AtomicCommitMode, AtomicCommitOutcome, write_save_container_atomic_timed,
};
use super::catalog::{
    SaveExpectedTarget, SaveFileRevision, SaveStorageRoot, read_save_file_revision,
};
use super::format::SaveHeader;
use super::schema::{build_persisted_world, collect_persisted_entities};
use super::state::{SaveLoadFailureKind, SaveLoadRequest, SaveLoadResult};

#[derive(Debug)]
enum SaveExecutionError {
    Serialize(String),
    OverwriteConfirmationRequired,
    RevisionUnavailable,
}

impl SaveExecutionError {
    const fn failure_kind(&self) -> SaveLoadFailureKind {
        match self {
            Self::Serialize(_) => SaveLoadFailureKind::SaveSerialize,
            Self::OverwriteConfirmationRequired => {
                SaveLoadFailureKind::OverwriteConfirmationRequired
            }
            Self::RevisionUnavailable => SaveLoadFailureKind::SaveWrite,
        }
    }
}

impl fmt::Display for SaveExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(error) => {
                write!(formatter, "DynamicWorld serialization failed: {error}")
            }
            Self::OverwriteConfirmationRequired => {
                formatter.write_str("save target revision changed; confirmation required")
            }
            Self::RevisionUnavailable => {
                formatter.write_str("existing save target has no stable revision")
            }
        }
    }
}

pub(super) fn save_world_system(world: &mut World) -> SaveLoadResult {
    let started = Instant::now();
    let master_seed = world
        .resource::<crate::world::map::GeneratedWorldLayoutResource>()
        .master_seed;
    let Some(SaveLoadRequest::Save {
        origin,
        slot,
        expected_target,
    }) = world.resource::<DispatchedSaveLoadRequest>().0.clone()
    else {
        error!("Save executor invoked without a dispatched save request");
        return SaveLoadResult::Failed(SaveLoadFailureKind::RequestRejected);
    };

    let save_path = world.resource::<SaveStorageRoot>().resolve(slot);
    let recheck = match read_save_file_revision(&save_path) {
        Ok(revision) => revision,
        Err(error) => {
            error!(
                "Failed to recheck save revision for {}: {error}",
                slot.player_label()
            );
            return SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite);
        }
    };

    if let Err(error) = recheck_expected_target(&expected_target, &recheck) {
        warn!(
            "Save recheck failed for {} ({origin:?}): {error}",
            slot.player_label()
        );
        return SaveLoadResult::Failed(error.failure_kind());
    }

    let serialize_started = Instant::now();
    let body = match serialize_world_body(world) {
        Ok(body) => body,
        Err(error) => {
            error!(
                "Failed to serialize world for {}: {error}",
                slot.player_label()
            );
            return SaveLoadResult::Failed(error.failure_kind());
        }
    };
    let serialize_ns = serialize_started.elapsed().as_nanos() as u64;
    let body_bytes = body.len();

    let commit_mode = match &expected_target {
        SaveExpectedTarget::Absent => AtomicCommitMode::NoReplace,
        SaveExpectedTarget::Exact(_) => AtomicCommitMode::Replace,
    };

    let commit = match write_save_container_atomic_timed(
        &save_path,
        SaveHeader::current(master_seed),
        &body,
        commit_mode,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            error!("Failed to save world to {}: {error}", slot.player_label());
            let kind = if matches!(error, super::atomic_file::AtomicWriteError::TargetExists) {
                SaveLoadFailureKind::OverwriteConfirmationRequired
            } else {
                SaveLoadFailureKind::SaveWrite
            };
            return SaveLoadResult::Failed(kind);
        }
    };

    let total_ns = started.elapsed().as_nanos() as u64;
    if let Some(mut metrics) = world.get_resource_mut::<super::SaveTransactionMetrics>() {
        metrics.record(super::SaveTransactionSample {
            body_bytes,
            serialize_ns,
            write_file_sync_ns: commit.timings.write_file_sync_ns,
            commit_directory_sync_ns: commit.timings.commit_directory_sync_ns,
            total_ns,
        });
    }

    let elapsed = started.elapsed();
    #[cfg(feature = "profiling")]
    let fixed_behavior_capture = world
        .get_resource::<crate::plugins::startup::PerfScenarioConfig>()
        .is_some_and(|config| config.behavior_case_as_str().is_some());
    #[cfg(not(feature = "profiling"))]
    let fixed_behavior_capture = false;
    if elapsed.as_millis() > 100 && !fixed_behavior_capture {
        warn!("Save of {} took {elapsed:?} (>100ms)", slot.player_label());
    } else {
        info!("World saved to {} in {elapsed:?}", slot.player_label());
    }

    match commit.outcome {
        AtomicCommitOutcome::Committed => SaveLoadResult::Succeeded,
        AtomicCommitOutcome::CommittedDurabilityUncertain => {
            SaveLoadResult::Failed(SaveLoadFailureKind::CommittedDurabilityUncertain)
        }
    }
}

fn recheck_expected_target(
    expected: &SaveExpectedTarget,
    actual: &SaveFileRevision,
) -> Result<(), SaveExecutionError> {
    match expected {
        SaveExpectedTarget::Absent => {
            if actual.exists {
                Err(SaveExecutionError::OverwriteConfirmationRequired)
            } else {
                Ok(())
            }
        }
        SaveExpectedTarget::Exact(expected_revision) => {
            if !expected_revision.exists {
                return Err(SaveExecutionError::RevisionUnavailable);
            }
            if !actual.exists || actual != expected_revision {
                Err(SaveExecutionError::OverwriteConfirmationRequired)
            } else {
                Ok(())
            }
        }
    }
}

/// Serializes the persisted simulation state without doing filesystem I/O.
fn serialize_world_body(world: &mut World) -> Result<String, SaveExecutionError> {
    let type_registry = world
        .resource::<bevy::ecs::reflect::AppTypeRegistry>()
        .clone();
    let registry = type_registry.read();

    let target_entities = collect_persisted_entities(world);

    let dynamic_world = build_persisted_world(world, &registry, target_entities.into_iter());

    dynamic_world
        .serialize(&registry)
        .map_err(|error| SaveExecutionError::Serialize(error.to_string()))
}

/// Builds an Exact/Absent expected target from authoritative metadata.
pub fn expected_target_from_revision(revision: SaveFileRevision) -> SaveExpectedTarget {
    if revision.exists {
        SaveExpectedTarget::Exact(revision)
    } else {
        SaveExpectedTarget::Absent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::atomic_file::temporary_save_path;
    use std::path::Path;

    #[test]
    fn temporary_paths_are_unique_for_the_same_save_file() {
        let path = Path::new("saves/world.scn.ron");
        assert_ne!(temporary_save_path(path), temporary_save_path(path));
    }

    #[test]
    fn absent_recheck_rejects_appeared_targets() {
        let err = recheck_expected_target(
            &SaveExpectedTarget::Absent,
            &SaveFileRevision {
                exists: true,
                length: 10,
                modified: None,
                file_identity: Some(1),
                prefix_fingerprint: 2,
            },
        )
        .unwrap_err();
        assert_eq!(
            err.failure_kind(),
            SaveLoadFailureKind::OverwriteConfirmationRequired
        );
    }

    #[test]
    fn exact_recheck_rejects_changed_revision() {
        let expected = SaveFileRevision {
            exists: true,
            length: 10,
            modified: None,
            file_identity: Some(1),
            prefix_fingerprint: 2,
        };
        let actual = SaveFileRevision {
            prefix_fingerprint: 3,
            ..expected.clone()
        };
        let err =
            recheck_expected_target(&SaveExpectedTarget::Exact(expected), &actual).unwrap_err();
        assert_eq!(
            err.failure_kind(),
            SaveLoadFailureKind::OverwriteConfirmationRequired
        );
    }
}
