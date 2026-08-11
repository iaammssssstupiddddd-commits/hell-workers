//! One-shot save timing capture for the `save-transaction` perf workload.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::env;

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

#[derive(SystemParam)]
pub(crate) struct SaveTransactionCaptureParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    capture: ResMut<'w, PerfCapture>,
    save_state: ResMut<'w, SaveLoadState>,
    metrics: Res<'w, SaveTransactionMetrics>,
    local: ResMut<'w, SaveTransactionCaptureState>,
    outcomes: MessageReader<'w, 's, SaveLoadOutcome>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn drive_save_transaction_capture_system(mut params: SaveTransactionCaptureParams) {
    if !params.config.enabled()
        || params.config.workload != PerfWorkload::SaveTransaction
        || !params.applied.complete()
    {
        return;
    }

    match params.capture.phase() {
        PerfCapturePhase::Measure if !params.local.completed => {
            if !params.local.issued {
                if params.save_state.is_idle() {
                    let _ = params.save_state.try_set(manual_save_request(
                        SaveSlotId::Manual1,
                        SAVE_TRANSACTION_DIALOG_SESSION,
                    ));
                    params.local.issued = true;
                }
                return;
            }
            for outcome in params.outcomes.read() {
                if !is_issued_manual_save_outcome(outcome) {
                    continue;
                }
                if outcome.result != SaveLoadResult::Succeeded {
                    error!(
                        "PERF_CAPTURE: issued save-transaction request did not succeed: {:?}",
                        outcome.result
                    );
                    params.capture.fail_capture();
                    params.exit.write(AppExit::error());
                    return;
                }
                let Some(sample) = params.metrics.last else {
                    error!("PERF_CAPTURE: save-transaction finished without phase metrics");
                    params.capture.fail_capture();
                    params.exit.write(AppExit::error());
                    return;
                };
                params.capture.store_save_transaction_sample(sample);
                #[cfg(feature = "profiling-memory")]
                params.capture.finish_memory_measurement();
                params.local.completed = true;
                return;
            }
        }
        _ => {}
    }
}

pub(crate) fn save_transaction_sample_kind_from_env() -> &'static str {
    match env::var("HW_PERF_SAVE_SAMPLE_KIND").as_deref() {
        Ok("preflight") => "preflight",
        Ok("measured") => "measured",
        _ => "measured",
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
