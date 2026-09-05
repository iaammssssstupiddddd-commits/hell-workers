use super::*;

pub(super) fn commit_deconstruction(
    world: &mut World,
    request: DeconstructionCommitRequest,
    prepared: PreparedCommit,
) -> Result<SuccessfulCommitMetrics, CommitFailure> {
    world
        .entity_mut(request.target)
        .insert(DeconstructionCommitClaim {
            world_epoch: request.world_epoch,
            order: request.order,
        });

    if !prepared_snapshot_still_matches(world, request.target, &prepared) {
        release_commit_claim(world, request);
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::OwnerMismatch,
            DeconstructionBlockReason::OwnerMismatch,
            TaskDiagnosticDomainMask::TASK
                .union(TaskDiagnosticDomainMask::TOPOLOGY)
                .union(TaskDiagnosticDomainMask::AVAILABILITY),
        ));
    }

    let terminal_outcomes = terminalize_exact_tasks(world, &prepared.terminal_requests);
    if terminal_outcomes
        .iter()
        .any(|outcome| outcome.result != ExactTaskTerminalResult::Applied)
    {
        release_commit_claim(world, request);
        let winner_failed = terminal_outcomes.iter().any(|outcome| {
            outcome.worker == request.worker
                && !matches!(
                    outcome.result,
                    ExactTaskTerminalResult::Applied | ExactTaskTerminalResult::BatchAborted
                )
        });
        return Err(if winner_failed {
            CommitFailure::untouched(DeconstructionCommitResult::StaleIdentity)
        } else {
            CommitFailure::blocked(
                DeconstructionCommitResult::UnsupportedTarget,
                DeconstructionBlockReason::UnsupportedTarget,
                TaskDiagnosticDomainMask::TASK,
            )
        });
    }

    let transport_cleanup = close_transport_requests_for_removed_owners(
        world,
        &prepared.removed_owners,
        &prepared.transport_requests,
    );
    assert_eq!(
        transport_cleanup,
        OwnerTransportCleanupResult::Applied,
        "prevalidated deconstruction transport cleanup changed during exclusive apply"
    );
    if prepared.kind == BuildingType::RestArea {
        let rest_release = release_rest_area_for_removed_owner(
            world,
            request.target,
            &prepared.recovery.rest_sources,
        );
        assert_eq!(
            rest_release,
            RestAreaReleaseResult::Applied,
            "prevalidated RestArea release changed during exclusive apply"
        );
    }
    if let Some(mut cache) = world.get_resource_mut::<SharedResourceCache>() {
        for &owner in &prepared.removed_owners {
            cache.clear_owner_reservations(owner);
        }
    }

    {
        let mut map = world.resource_mut::<WorldMap>();
        match prepared.kind {
            BuildingType::Door => {
                for grid in &prepared.owner_snapshot.door_grids {
                    let cleared = map.clear_door_if_owned(*grid, request.target);
                    debug_assert!(cleared, "validated door owner changed in exclusive commit");
                }
            }
            BuildingType::Floor => {
                for grid in &prepared.owner_snapshot.floor_grids {
                    let cleared = map.clear_floor_if_owned(*grid, request.target);
                    debug_assert!(cleared, "validated floor owner changed in exclusive commit");
                }
            }
            BuildingType::Bridge => {
                for grid in &prepared.owner_snapshot.bridge_grids {
                    let cleared = map.clear_bridge_if_owned(*grid, request.target);
                    debug_assert!(
                        cleared,
                        "validated bridge owner changed in exclusive commit"
                    );
                }
            }
            BuildingType::SoulSpa | BuildingType::OutdoorLamp => {
                for grid in &prepared.owner_snapshot.building_grids {
                    let cleared = map.clear_building_if_owned(*grid, request.target);
                    debug_assert!(
                        cleared,
                        "validated passable building owner changed in exclusive commit"
                    );
                }
            }
            _ => {
                for grid in &prepared.owner_snapshot.building_grids {
                    let cleared = map.clear_building_occupancy_if_owned(*grid, request.target);
                    debug_assert!(
                        cleared,
                        "validated deconstruction owner changed in exclusive commit"
                    );
                }
            }
        }
        for companion in &prepared.recovery.companions_to_remove {
            for grid in &companion.owner_snapshot.stockpile_grids {
                let cleared = map.clear_stockpile_tile_if_owned(*grid, companion.entity);
                debug_assert!(
                    cleared,
                    "validated companion stockpile owner changed in exclusive commit"
                );
            }
        }
    }

    let dirty_grids = prepared
        .owner_snapshot
        .building_grids
        .iter()
        .chain(&prepared.owner_snapshot.floor_grids)
        .copied()
        .collect::<Vec<_>>();
    if let Some(mut room_detection) = world.get_resource_mut::<RoomDetectionState>() {
        room_detection.mark_dirty_many(dirty_grids.iter().copied());
    }
    if matches!(prepared.kind, BuildingType::Wall | BuildingType::Door)
        && let Some(mut wall_connections) =
            world.get_resource_mut::<hw_visual::wall_connection::WallConnectionDirty>()
    {
        wall_connections.mark_removed(dirty_grids.iter().copied());
    }

    #[cfg(feature = "profiling")]
    let recovery_items_spawned = prepared.recovery.spawned_items.len() as u64;
    apply_facility_recovery(world, &prepared.recovery);

    for visual in prepared.visual_entities {
        if let Ok(entity) = world.get_entity_mut(visual) {
            entity.despawn();
        }
    }
    for tile in prepared.soul_spa_tiles {
        if let Ok(entity) = world.get_entity_mut(tile.entity) {
            entity.despawn();
        }
    }
    for order in prepared.orders_to_despawn {
        if let Ok(order) = world.get_entity_mut(order) {
            order.despawn();
        }
    }
    if let Ok(target) = world.get_entity_mut(request.target) {
        target.despawn();
    }
    if matches!(
        prepared.kind,
        BuildingType::SoulSpa | BuildingType::OutdoorLamp
    ) && let Some(mut dirty) = world.get_resource_mut::<EnergyUpdateDirty>()
    {
        dirty.request_full_rebuild();
    }

    Ok(SuccessfulCommitMetrics {
        #[cfg(feature = "profiling")]
        recovery_items_spawned,
    })
}

#[cfg(feature = "profiling")]
pub(super) fn record_commit_validation_pass(world: &mut World) {
    if let Some(mut metrics) = world.get_resource_mut::<DeconstructionPerfMetrics>() {
        metrics.commit_validation_passes = metrics.commit_validation_passes.saturating_add(1);
    }
}

#[cfg(feature = "profiling")]
pub(super) fn record_successful_commit_metrics(
    world: &mut World,
    commit_metrics: SuccessfulCommitMetrics,
    elapsed: std::time::Duration,
) {
    if let Some(mut metrics) = world.get_resource_mut::<DeconstructionPerfMetrics>() {
        metrics.successful_cleanup_transactions =
            metrics.successful_cleanup_transactions.saturating_add(1);
        metrics.recovery_items_spawned = metrics
            .recovery_items_spawned
            .saturating_add(commit_metrics.recovery_items_spawned);
        metrics.successful_transaction_elapsed_ns = metrics
            .successful_transaction_elapsed_ns
            .saturating_add(elapsed.as_nanos());
    }
}

fn prepared_snapshot_still_matches(
    world: &mut World,
    target: Entity,
    prepared: &PreparedCommit,
) -> bool {
    let world_map_matches = world.get_resource::<WorldMap>().is_some_and(|map| {
        map.snapshot_owner(target) == prepared.owner_snapshot
            && prepared
                .recovery
                .companions_to_remove
                .iter()
                .all(|companion| map.snapshot_owner(companion.entity) == companion.owner_snapshot)
    });
    if !world_map_matches {
        return false;
    }
    let soul_spa_tiles_match =
        preflight::prepare_soul_spa_tiles(world, target, prepared.kind, &prepared.owner_snapshot)
            .is_ok_and(|tiles| tiles == prepared.soul_spa_tiles);
    if !soul_spa_tiles_match {
        return false;
    }
    let power_topology_matches = preflight::prepare_power_topology(world, target, prepared.kind)
        .is_ok_and(|topology| topology == prepared.power_topology);
    if !power_topology_matches
        || preflight::building_visual_entities(world, target) != prepared.visual_entities
    {
        return false;
    }
    recovery_plan_still_matches(world, &prepared.recovery, target)
        && transport_requests_referencing_removed_owners(world, &prepared.removed_owners)
            == prepared.transport_requests
        && (prepared.kind != BuildingType::RestArea
            || rest_area_relationship_sources(world, target) == prepared.recovery.rest_sources)
}

pub(super) fn release_commit_claim(world: &mut World, request: DeconstructionCommitRequest) {
    if let Ok(mut target) = world.get_entity_mut(request.target)
        && target
            .get::<DeconstructionCommitClaim>()
            .is_some_and(|claim| {
                claim.world_epoch == request.world_epoch && claim.order == request.order
            })
    {
        target.remove::<DeconstructionCommitClaim>();
    }
}
