use super::*;

pub(super) fn handle_commit_failure(
    world: &mut World,
    request: DeconstructionCommitRequest,
    failure: CommitFailure,
) {
    if failure.result != DeconstructionCommitResult::StaleWorld {
        commit::release_commit_claim(world, request);
    }
    if failure.result == DeconstructionCommitResult::StaleWorld {
        return;
    }
    if failure.result == DeconstructionCommitResult::StaleIdentity {
        terminalize_commit_worker(
            world,
            request,
            ExactTaskTerminalDisposition::Abort {
                emit_abandoned: false,
            },
        );
        return;
    }
    if failure.discard_orphaned_order {
        let terminalized = prepare_owner_task_terminals(world, &[request.order], None, &[])
            .ok()
            .map(|terminal_requests| terminalize_exact_tasks(world, &terminal_requests))
            .is_some_and(|outcomes| {
                outcomes
                    .iter()
                    .all(|outcome| outcome.result == ExactTaskTerminalResult::Applied)
            });
        if terminalized {
            remove_orphaned_pending(world, request.target, request.order);
            if let Ok(order) = world.get_entity_mut(request.order) {
                order.despawn();
            }
        }
        return;
    }
    let terminal_result = terminalize_commit_worker(
        world,
        request,
        ExactTaskTerminalDisposition::Abort {
            emit_abandoned: false,
        },
    );
    if let Some((reason, domains)) = failure.blocker
        && terminal_result == ExactTaskTerminalResult::Applied
    {
        let blocker =
            deconstruction_blocker_after_worker_cleanup(world, request.order, reason, domains);
        if let Ok(mut order) = world.get_entity_mut(request.order) {
            order.insert(blocker);
        }
    }
}

fn remove_orphaned_pending(world: &mut World, target: Entity, discarded_order: Entity) {
    let Some(pending_order) = world
        .get::<DeconstructionPending>(target)
        .map(|pending| pending.order)
    else {
        return;
    };
    let pending_is_live = world.get::<DeconstructionOrder>(pending_order).is_some()
        && world
            .get::<Designation>(pending_order)
            .is_some_and(|designation| designation.work_type == WorkType::Deconstruct)
        && world
            .get::<TargetDeconstructionRoot>(pending_order)
            .is_some_and(|relation| relation.0 == target);
    if (pending_order == discarded_order || !pending_is_live)
        && let Ok(mut target) = world.get_entity_mut(target)
    {
        target.remove::<DeconstructionPending>();
    }
}

fn deconstruction_blocker_after_worker_cleanup(
    world: &World,
    order: Entity,
    reason: DeconstructionBlockReason,
    domains: TaskDiagnosticDomainMask,
) -> DeconstructionBlocker {
    let Some(revisions) = world.get_resource::<TaskDiagnosticInputRevisions>() else {
        return DeconstructionBlocker::pending(reason, domains);
    };
    let mut stamp = revisions.stamp_for(order);
    if domains.contains(TaskDiagnosticDomainMask::TASK) {
        // Removing the exact WorkingOn edge changes TaskWorkers once. Rebase
        // over that known cleanup bump; any additional task-domain change
        // before the next sync still makes this blocker stale.
        stamp.task = stamp.task.wrapping_add(1);
    }
    DeconstructionBlocker::armed(reason, domains, stamp)
}

pub(super) fn terminalize_commit_worker(
    world: &mut World,
    request: DeconstructionCommitRequest,
    disposition: ExactTaskTerminalDisposition,
) -> ExactTaskTerminalResult {
    terminalize_exact_tasks(
        world,
        &[ExactTaskTerminalRequest {
            worker: request.worker,
            expected_identity: request.identity,
            expectation: ExactTaskExpectation::DeconstructionAwaitingCommit {
                order: request.order,
                target: request.target,
            },
            disposition,
        }],
    )[0]
    .result
}

pub(super) fn related_deconstruction_orders_for_target(
    world: &World,
    target: Entity,
    canonical_order: Entity,
) -> Result<Vec<Entity>, RelatedOrdersFailure> {
    let relations = world
        .get::<hw_jobs::DeconstructionOrders>(target)
        .ok_or(RelatedOrdersFailure::CanonicalInvalid)?;
    let mut orders = relations.iter().copied().collect::<Vec<_>>();
    orders.sort_unstable_by_key(|entity| entity.to_bits());
    if !orders.contains(&canonical_order) {
        return Err(RelatedOrdersFailure::CanonicalInvalid);
    }
    let canonical_is_valid = world.get::<DeconstructionOrder>(canonical_order).is_some()
        && world
            .get::<Designation>(canonical_order)
            .is_some_and(|designation| designation.work_type == WorkType::Deconstruct)
        && world
            .get::<TargetDeconstructionRoot>(canonical_order)
            .is_some_and(|relation| relation.0 == target);
    if !canonical_is_valid {
        return Err(RelatedOrdersFailure::CanonicalInvalid);
    }
    for &sibling in orders.iter().filter(|&&order| order != canonical_order) {
        let sibling_is_owned_order = world.get::<DeconstructionOrder>(sibling).is_some()
            && world
                .get::<TargetDeconstructionRoot>(sibling)
                .is_some_and(|relation| relation.0 == target);
        if !sibling_is_owned_order {
            return Err(RelatedOrdersFailure::MalformedSibling);
        }
    }
    Ok(orders)
}

pub(super) fn move_task_references(world: &mut World, target: Entity) -> bool {
    let durable_task_exists = {
        let mut query = world.query::<&MovePlantTask>();
        query.iter(world).any(|task| task.building == target)
    };
    if durable_task_exists {
        return true;
    }
    let mut query = world.query::<&AssignedTask>();
    query
        .iter(world)
        .any(|task| matches!(task, AssignedTask::MovePlant(data) if data.building == target))
}
