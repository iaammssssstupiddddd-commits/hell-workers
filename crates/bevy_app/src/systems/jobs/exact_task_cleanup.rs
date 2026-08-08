//! Shared preflight for root-owned transactions that remove task references.

use std::collections::HashSet;

use bevy::prelude::*;
use hw_core::relationships::{TaskWorkers, WorkingOn};
use hw_jobs::{ActiveTaskIdentity, AssignedTask};
use hw_soul_ai::{ExactTaskExpectation, ExactTaskTerminalDisposition, ExactTaskTerminalRequest};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompletingExactTask {
    pub worker: Entity,
    pub identity: ActiveTaskIdentity,
    pub expectation: ExactTaskExpectation,
}

/// Snapshots every worker whose task shell references an owner that will be
/// removed. The returned exact requests are safe to apply as one all-or-none
/// batch through `terminalize_exact_tasks`.
pub(crate) fn prepare_owner_task_terminals(
    world: &mut World,
    cleanup_references: &[Entity],
    completing: Option<CompletingExactTask>,
    preserve_loaded_carriers: &[Entity],
) -> Result<Vec<ExactTaskTerminalRequest>, ()> {
    let mut query = world.query::<(
        Entity,
        &AssignedTask,
        Option<&ActiveTaskIdentity>,
        Option<&WorkingOn>,
    )>();
    let mut requests = Vec::new();
    for (worker, task, identity, working_on) in query.iter(world) {
        let Some(reference) = cleanup_references.iter().copied().find(|&reference| {
            task.references_entity(reference)
                || identity.is_some_and(|identity| {
                    identity.assignment_entity == reference
                        || identity.current_target_entity == reference
                })
                || working_on.is_some_and(|working_on| working_on.0 == reference)
        }) else {
            continue;
        };
        let Some(identity) = identity.copied() else {
            return Err(());
        };
        let is_completing = completing.is_some_and(|completing| completing.worker == worker);
        let preserve_wheelbarrow_cargo = !is_completing
            && matches!(
                task,
                AssignedTask::HaulWithWheelbarrow(data)
                    if preserve_loaded_carriers.contains(&data.wheelbarrow)
            );
        requests.push(ExactTaskTerminalRequest {
            worker,
            expected_identity: identity,
            expectation: if is_completing {
                completing
                    .expect("completing worker requires an exact expectation")
                    .expectation
            } else {
                ExactTaskExpectation::References(reference)
            },
            disposition: if is_completing {
                ExactTaskTerminalDisposition::Complete
            } else if preserve_wheelbarrow_cargo {
                ExactTaskTerminalDisposition::AbortPreservingWheelbarrowCargo {
                    emit_abandoned: false,
                }
            } else {
                ExactTaskTerminalDisposition::Abort {
                    emit_abandoned: false,
                }
            },
        });
    }
    requests.sort_unstable_by_key(|request| request.worker.to_bits());

    let request_workers = requests
        .iter()
        .map(|request| request.worker)
        .collect::<HashSet<_>>();
    for &reference in cleanup_references {
        if let Some(workers) = world.get::<TaskWorkers>(reference)
            && workers
                .iter()
                .any(|worker| !request_workers.contains(worker))
        {
            return Err(());
        }
    }
    if let Some(completing) = completing
        && (requests
            .iter()
            .filter(|terminal| terminal.worker == completing.worker)
            .count()
            != 1
            || requests
                .iter()
                .find(|terminal| terminal.worker == completing.worker)
                .is_none_or(|terminal| terminal.expected_identity != completing.identity))
    {
        return Err(());
    }

    Ok(requests)
}
