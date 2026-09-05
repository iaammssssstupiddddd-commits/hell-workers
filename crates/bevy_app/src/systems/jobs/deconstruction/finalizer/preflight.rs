use super::*;

pub(super) fn validate_commit(
    world: &mut World,
    current_epoch: u64,
    request: DeconstructionCommitRequest,
) -> Result<PreparedCommit, CommitFailure> {
    if request.world_epoch != current_epoch {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleWorld,
        ));
    }
    if request.identity.assignment_entity != request.order
        || request.identity.current_target_entity != request.order
        || request.identity.current_work_type != WorkType::Deconstruct
    {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleIdentity,
        ));
    }
    if world.get::<DeconstructionOrder>(request.order).is_none() {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let identity_matches = world.get::<DamnedSoul>(request.worker).is_some()
        && world.get::<Transform>(request.worker).is_some()
        && world
            .get::<ActiveTaskIdentity>(request.worker)
            .is_some_and(|identity| *identity == request.identity)
        && world
            .get::<WorkingOn>(request.worker)
            .is_some_and(|working| request.identity.matches_working_on(Some(working.0)))
        && world
            .get::<AssignedTask>(request.worker)
            .is_some_and(|task| {
                matches!(
                    task,
                    AssignedTask::Deconstruct(data)
                        if data.order == request.order
                            && data.target == request.target
                            && data.phase == DeconstructPhase::AwaitingCommit
                ) && task.work_type() == Some(request.identity.current_work_type)
            });
    if !identity_matches {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleIdentity,
        ));
    }
    if world
        .get::<Designation>(request.order)
        .is_none_or(|designation| designation.work_type != WorkType::Deconstruct)
    {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let Some(current_target) = world
        .get::<TargetDeconstructionRoot>(request.order)
        .map(|target| target.0)
    else {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    };
    if current_target != request.target {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    if world.get_entity(request.target).is_err() {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    if world
        .get::<DeconstructionPending>(request.target)
        .is_none_or(|pending| pending.order != request.order)
    {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let orders_to_despawn = match failure::related_deconstruction_orders_for_target(
        world,
        request.target,
        request.order,
    ) {
        Ok(orders) => orders,
        Err(RelatedOrdersFailure::CanonicalInvalid) => {
            return Err(CommitFailure::orphaned_order(
                DeconstructionCommitResult::StaleTarget,
            ));
        }
        Err(RelatedOrdersFailure::MalformedSibling) => {
            return Err(CommitFailure::blocked(
                DeconstructionCommitResult::StaleTarget,
                DeconstructionBlockReason::StaleTarget,
                TaskDiagnosticDomainMask::TASK,
            ));
        }
    };
    if world
        .get::<DeconstructionCommitClaim>(request.target)
        .is_some_and(|claim| {
            claim.world_epoch != request.world_epoch || claim.order != request.order
        })
    {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::Duplicate,
        ));
    }
    if world.get::<MovePlanned>(request.target).is_some()
        || world.get::<PendingBuildingMove>(request.target).is_some()
        || failure::move_task_references(world, request.target)
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::Moving,
            DeconstructionBlockReason::Moving,
            TaskDiagnosticDomainMask::TASK,
        ));
    }
    if world
        .get::<DeconstructionBlocker>(request.order)
        .is_some_and(|blocker| blocker.active)
    {
        return Err(CommitFailure::untouched(
            DeconstructionCommitResult::UnsupportedTarget,
        ));
    }

    let Some((kind, is_provisional)) = world
        .get::<Building>(request.target)
        .map(|building| (building.kind, building.is_provisional))
    else {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    };
    let Some(target_position) = world
        .get::<Transform>(request.target)
        .map(|transform| transform.translation.truncate())
    else {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    };
    if is_provisional {
        return Err(CommitFailure::orphaned_order(
            DeconstructionCommitResult::StaleTarget,
        ));
    }
    let resolved = resolve_deconstruction_target(world, request.target).map_err(|_| {
        CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        )
    })?;
    if resolved.root != request.target || resolved.class.building_type() != kind {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::OwnerMismatch,
            DeconstructionBlockReason::OwnerMismatch,
            TaskDiagnosticDomainMask::TASK,
        ));
    }
    if !supports_deconstruction_cleanup(kind)
        || !designation_target_shape_is_supported(world, resolved)
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ));
    }

    let (owner_snapshot, anchor) = {
        let Some(map) = world.get_resource::<WorldMap>() else {
            return Err(CommitFailure::blocked(
                DeconstructionCommitResult::OwnerMismatch,
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
            ));
        };
        let owner_snapshot = map.snapshot_owner(request.target);
        let anchor = WorldMap::world_to_grid(target_position);
        let order_grid_matches = world
            .get::<Transform>(request.order)
            .is_some_and(|transform| {
                WorldMap::world_to_grid(transform.translation.truncate()) == anchor
            });
        let owner_is_exact = order_grid_matches
            && target_owner_snapshot_is_exact(map, kind, &owner_snapshot, anchor);
        if !owner_is_exact {
            return Err(CommitFailure::blocked(
                DeconstructionCommitResult::OwnerMismatch,
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
            ));
        }
        (owner_snapshot, anchor)
    };
    let soul_spa_tiles = prepare_soul_spa_tiles(world, request.target, kind, &owner_snapshot)?;
    let power_topology = prepare_power_topology(world, request.target, kind)?;
    let visual_entities = building_visual_entities(world, request.target);
    let recovery = prepare_facility_recovery(
        world,
        request.target,
        kind,
        &owner_snapshot,
        anchor,
        deconstruction_salvage(kind),
    )
    .map_err(recovery_failure)?;
    if !recovery.spawned_items.is_empty()
        && world.get_resource::<ResourceItemVisualHandles>().is_none()
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ));
    }
    let mut removed_owners = recovery.removed_owner_entities(request.target);
    removed_owners.extend(soul_spa_tiles.iter().map(|tile| tile.entity));
    removed_owners.sort_unstable_by_key(|entity| entity.to_bits());
    removed_owners.dedup();
    let transport_requests = transport_requests_referencing_removed_owners(world, &removed_owners);
    if transport_requests
        .iter()
        .any(|request| removed_owners.contains(request))
    {
        return Err(CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ));
    }

    let mut cleanup_references =
        Vec::with_capacity(1 + orders_to_despawn.len() + transport_requests.len());
    cleanup_references.push(request.target);
    cleanup_references.extend(orders_to_despawn.iter().copied());
    cleanup_references.extend(transport_requests.iter().copied());
    cleanup_references.extend(recovery.cleanup_reference_entities());
    cleanup_references.extend(soul_spa_tiles.iter().map(|tile| tile.entity));
    cleanup_references.sort_unstable_by_key(|entity| entity.to_bits());
    cleanup_references.dedup();
    let preserve_loaded_carriers = recovery
        .wheelbarrows
        .iter()
        .map(|carrier| carrier.entity)
        .collect::<Vec<_>>();
    let terminal_requests = prepare_owner_task_terminals(
        world,
        &cleanup_references,
        Some(CompletingExactTask {
            worker: request.worker,
            identity: request.identity,
            expectation: ExactTaskExpectation::DeconstructionAwaitingCommit {
                order: request.order,
                target: request.target,
            },
        }),
        &preserve_loaded_carriers,
    )
    .map_err(|_| {
        CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        )
    })?;

    Ok(PreparedCommit {
        kind,
        owner_snapshot,
        soul_spa_tiles,
        power_topology,
        visual_entities,
        recovery,
        removed_owners,
        terminal_requests,
        transport_requests,
        orders_to_despawn,
    })
}

fn target_owner_snapshot_is_exact(
    map: &WorldMap,
    kind: BuildingType,
    snapshot: &WorldMapOwnerSnapshot,
    anchor: (i32, i32),
) -> bool {
    if !snapshot.stockpile_grids.is_empty() {
        return false;
    }

    let exact_single = |grids: &[(i32, i32)]| grids == [anchor];
    match kind {
        BuildingType::Wall
        | BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::OutdoorLamp => {
            exact_single(&snapshot.building_grids)
                && snapshot.floor_grids.is_empty()
                && snapshot.door_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Door => {
            exact_single(&snapshot.building_grids)
                && exact_single(&snapshot.door_grids)
                && map.door_state(anchor.0, anchor.1).is_some()
                && snapshot.floor_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Floor => {
            exact_single(&snapshot.floor_grids)
                && snapshot.building_grids.is_empty()
                && snapshot.door_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking
        | BuildingType::SoulSpa => {
            exact_rectangle(&snapshot.building_grids, 2, 2)
                && snapshot.building_grids.contains(&anchor)
                && snapshot.floor_grids.is_empty()
                && snapshot.door_grids.is_empty()
                && snapshot.bridge_grids.is_empty()
        }
        BuildingType::Bridge => {
            exact_rectangle(&snapshot.building_grids, 2, 5)
                && snapshot.building_grids.contains(&anchor)
                && snapshot.bridge_grids == snapshot.building_grids
                && snapshot.floor_grids.is_empty()
                && snapshot.door_grids.is_empty()
        }
    }
}

fn exact_rectangle(grids: &[(i32, i32)], width: i32, height: i32) -> bool {
    if grids.len() != (width * height) as usize {
        return false;
    }
    let Some(min_x) = grids.iter().map(|grid| grid.0).min() else {
        return false;
    };
    let Some(min_y) = grids.iter().map(|grid| grid.1).min() else {
        return false;
    };
    (min_y..min_y + height)
        .flat_map(|y| (min_x..min_x + width).map(move |x| (x, y)))
        .all(|grid| {
            grids
                .binary_search_by_key(&(grid.1, grid.0), |&(x, y)| (y, x))
                .is_ok()
        })
}

pub(super) fn prepare_soul_spa_tiles(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
    owner_snapshot: &WorldMapOwnerSnapshot,
) -> Result<Vec<SoulSpaTileSnapshot>, CommitFailure> {
    let mut query = world.query::<(
        Entity,
        &SoulSpaTile,
        Option<&ChildOf>,
        Option<&Designation>,
        Option<&hw_jobs::TaskSlots>,
    )>();
    let mut snapshots = Vec::new();
    for (entity, tile, parent, designation, slots) in query.iter(world) {
        let child_of_target = parent.is_some_and(|parent| parent.parent() == target);
        if tile.parent_site != target && !child_of_target {
            continue;
        }
        if kind != BuildingType::SoulSpa
            || tile.parent_site != target
            || parent.is_some_and(|parent| parent.parent() != target)
            || designation
                .is_none_or(|designation| designation.work_type != WorkType::GeneratePower)
            || slots.is_none_or(|slots| slots.max != 1)
        {
            return Err(owner_mismatch_failure());
        }
        snapshots.push(SoulSpaTileSnapshot {
            entity,
            grid: tile.grid_pos,
        });
    }
    snapshots.sort_unstable_by_key(|snapshot| snapshot.entity.to_bits());

    if kind != BuildingType::SoulSpa {
        return snapshots
            .is_empty()
            .then_some(snapshots)
            .ok_or_else(owner_mismatch_failure);
    }
    if world
        .get::<SoulSpaSite>(target)
        .is_none_or(|site| site.phase != SoulSpaPhase::Operational)
        || snapshots.len() != 4
    {
        return Err(owner_mismatch_failure());
    }
    let mut tile_grids = snapshots.iter().map(|tile| tile.grid).collect::<Vec<_>>();
    tile_grids.sort_unstable_by_key(|&(x, y)| (y, x));
    tile_grids.dedup();
    if tile_grids != owner_snapshot.building_grids {
        return Err(owner_mismatch_failure());
    }
    Ok(snapshots)
}

pub(super) fn prepare_power_topology(
    world: &mut World,
    target: Entity,
    kind: BuildingType,
) -> Result<PowerTopologySnapshot, CommitFailure> {
    let snapshot = PowerTopologySnapshot {
        generator_grid: world.get::<GeneratesFor>(target).map(|relation| relation.0),
        consumer_grid: world.get::<ConsumesFrom>(target).map(|relation| relation.0),
    };
    let marker_shape_matches = match kind {
        BuildingType::SoulSpa => {
            world.get::<PowerGenerator>(target).is_some()
                && world.get::<PowerConsumer>(target).is_none()
                && snapshot.consumer_grid.is_none()
        }
        BuildingType::OutdoorLamp => {
            world.get::<PowerConsumer>(target).is_some()
                && world.get::<PowerGenerator>(target).is_none()
                && snapshot.generator_grid.is_none()
        }
        _ => {
            world.get::<PowerGenerator>(target).is_none()
                && world.get::<PowerConsumer>(target).is_none()
                && snapshot == PowerTopologySnapshot::default()
        }
    };
    if !marker_shape_matches || !power_topology_reverse_edges_match(world, target, snapshot) {
        return Err(owner_mismatch_failure());
    }
    Ok(snapshot)
}

fn power_topology_reverse_edges_match(
    world: &mut World,
    target: Entity,
    snapshot: PowerTopologySnapshot,
) -> bool {
    let mut generator_query = world.query::<(Entity, &GridGenerators)>();
    let mut generator_grids = generator_query
        .iter(world)
        .filter_map(|(grid, generators)| {
            generators
                .iter()
                .any(|source| *source == target)
                .then_some(grid)
        })
        .collect::<Vec<_>>();
    generator_grids.sort_unstable_by_key(|entity| entity.to_bits());
    let mut consumer_query = world.query::<(Entity, &GridConsumers)>();
    let mut consumer_grids = consumer_query
        .iter(world)
        .filter_map(|(grid, consumers)| {
            consumers
                .iter()
                .any(|source| *source == target)
                .then_some(grid)
        })
        .collect::<Vec<_>>();
    consumer_grids.sort_unstable_by_key(|entity| entity.to_bits());

    generator_grids == snapshot.generator_grid.into_iter().collect::<Vec<_>>()
        && consumer_grids == snapshot.consumer_grid.into_iter().collect::<Vec<_>>()
}

pub(super) fn building_visual_entities(world: &mut World, target: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &Building3dVisual)>();
    let mut visuals = query
        .iter(world)
        .filter_map(|(entity, visual)| (visual.owner == target).then_some(entity))
        .collect::<Vec<_>>();
    visuals.sort_unstable_by_key(|entity| entity.to_bits());
    visuals
}

fn owner_mismatch_failure() -> CommitFailure {
    CommitFailure::blocked(
        DeconstructionCommitResult::OwnerMismatch,
        DeconstructionBlockReason::OwnerMismatch,
        TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
    )
}

fn recovery_failure(failure: RecoveryPlanFailure) -> CommitFailure {
    match failure {
        RecoveryPlanFailure::OwnerMismatch => CommitFailure::blocked(
            DeconstructionCommitResult::OwnerMismatch,
            DeconstructionBlockReason::OwnerMismatch,
            TaskDiagnosticDomainMask::TASK.union(TaskDiagnosticDomainMask::TOPOLOGY),
        ),
        RecoveryPlanFailure::NoSafeRecovery => CommitFailure::blocked(
            DeconstructionCommitResult::NoSafeRecovery,
            DeconstructionBlockReason::NoSafeRecovery,
            TaskDiagnosticDomainMask::TOPOLOGY.union(TaskDiagnosticDomainMask::AVAILABILITY),
        ),
        RecoveryPlanFailure::InconsistentMixerInventory => CommitFailure::blocked(
            DeconstructionCommitResult::InconsistentMixerInventory,
            DeconstructionBlockReason::InconsistentMixerInventory,
            TaskDiagnosticDomainMask::AVAILABILITY,
        ),
        RecoveryPlanFailure::UnsupportedTarget => CommitFailure::blocked(
            DeconstructionCommitResult::UnsupportedTarget,
            DeconstructionBlockReason::UnsupportedTarget,
            TaskDiagnosticDomainMask::TASK,
        ),
    }
}
