//! One-shot save timing capture for the `save-transaction` perf workload.

use bevy::prelude::*;

use crate::systems::save::{
    SaveLoadOperation, SaveLoadOutcome, SaveLoadOutcomeSource, SaveLoadResult, SaveLoadState,
    SaveRequestOrigin, SaveTransactionMetrics, manual_save_request,
};
use hw_core::SaveSlotId;

use super::config::{PerfScenarioConfig, PerfWorkload};
use super::fixture::PerfScenarioApplied;
use super::{PerfCapture, PerfCapturePhase};

#[derive(Resource, Default)]
pub(crate) struct SaveTransactionCaptureState {
    issued: bool,
    completed: bool,
}

const SAVE_TRANSACTION_DIALOG_SESSION: u64 = 1;

fn is_issued_manual_save_outcome(outcome: &SaveLoadOutcome) -> bool {
    outcome.operation == SaveLoadOperation::Save
        && outcome.target == SaveSlotId::Manual1.player_label()
        && matches!(
            outcome.source,
            SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog {
                dialog_session: SAVE_TRANSACTION_DIALOG_SESSION,
            })
        )
}

pub(crate) fn drive_save_transaction_capture_system(
    config: Res<PerfScenarioConfig>,
    applied: Res<PerfScenarioApplied>,
    mut capture: ResMut<PerfCapture>,
    mut save_state: ResMut<SaveLoadState>,
    metrics: Res<SaveTransactionMetrics>,
    mut local: ResMut<SaveTransactionCaptureState>,
    mut outcomes: MessageReader<SaveLoadOutcome>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.enabled() || config.workload != PerfWorkload::SaveTransaction || !applied.complete()
    {
        return;
    }

    match capture.phase() {
        PerfCapturePhase::Measure if !local.completed => {
            if !local.issued {
                if save_state.is_idle() {
                    let _ = save_state.try_set(manual_save_request(
                        SaveSlotId::Manual1,
                        SAVE_TRANSACTION_DIALOG_SESSION,
                    ));
                    local.issued = true;
                }
                return;
            }
            for outcome in outcomes.read() {
                if !is_issued_manual_save_outcome(outcome) {
                    continue;
                }
                if outcome.result != SaveLoadResult::Succeeded {
                    error!(
                        "PERF_CAPTURE: issued save-transaction request did not succeed: {:?}",
                        outcome.result
                    );
                    capture.fail_capture();
                    exit.write(AppExit::error());
                    return;
                }
                let Some(sample) = metrics.last else {
                    error!("PERF_CAPTURE: save-transaction finished without phase metrics");
                    capture.fail_capture();
                    exit.write(AppExit::error());
                    return;
                };
                capture.store_save_transaction_sample(sample);
                #[cfg(feature = "profiling-memory")]
                capture.finish_memory_measurement();
                local.completed = true;
                return;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::{LoadRequestOrigin, SaveLoadFailureKind};

    fn outcome(
        source: SaveLoadOutcomeSource,
        target: &str,
        result: SaveLoadResult,
    ) -> SaveLoadOutcome {
        SaveLoadOutcome {
            operation: SaveLoadOperation::Save,
            target: target.to_owned(),
            result,
            source,
        }
    }

    #[test]
    fn accepts_only_the_issued_manual_slot_outcome() {
        assert!(is_issued_manual_save_outcome(&outcome(
            SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog {
                dialog_session: SAVE_TRANSACTION_DIALOG_SESSION,
            }),
            SaveSlotId::Manual1.player_label(),
            SaveLoadResult::Succeeded,
        )));

        assert!(!is_issued_manual_save_outcome(&outcome(
            SaveLoadOutcomeSource::Autosave,
            SaveSlotId::Manual1.player_label(),
            SaveLoadResult::Succeeded,
        )));
        assert!(!is_issued_manual_save_outcome(&outcome(
            SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog {
                dialog_session: SAVE_TRANSACTION_DIALOG_SESSION + 1,
            }),
            SaveSlotId::Manual1.player_label(),
            SaveLoadResult::Failed(SaveLoadFailureKind::CommittedDurabilityUncertain),
        )));
        assert!(!is_issued_manual_save_outcome(&outcome(
            SaveLoadOutcomeSource::Load(LoadRequestOrigin::NormalCatalog {
                dialog_session: SAVE_TRANSACTION_DIALOG_SESSION,
            }),
            SaveSlotId::Manual2.player_label(),
            SaveLoadResult::Succeeded,
        )));
    }
}

pub(crate) fn save_transaction_sample_kind_from_env() -> &'static str {
    match env::var("HW_PERF_SAVE_SAMPLE_KIND").as_deref() {
        Ok("preflight") => "preflight",
        Ok("measured") => "measured",
        _ => "measured",
    }
}
use std::env;
