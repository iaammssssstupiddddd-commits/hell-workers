//! Root bridge from game-specific change sources to shared task revisions.

use std::collections::{HashMap, HashSet};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::FATIGUE_THRESHOLD;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarOperation, FamiliarPolicy};
use hw_core::relationships::{
    CommandedBy, Commanding, DeliveringTo, IncomingDeliveries, LoadedIn, LoadedItems, ManagedBy,
    ManagedTasks, ParkedAt, PushedBy, StoredIn, StoredItems, TaskWorkers,
};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use hw_energy::constants::DREAM_GENERATE_ASSIGN_THRESHOLD;
use hw_energy::{
    ConsumesFrom, GeneratesFor, PowerConsumer, PowerGenerator, SoulSpaSite, SoulSpaTile,
};
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    AssignedTask, Blueprint, BonePile, BridgeMarker, Building, DeconstructionBlocker,
    DeconstructionOrder, DeconstructionOrders, DeconstructionPending, Designation, Door,
    MovePlanned, MovePlantTask, PendingBuildingMove, RestArea, SandPile, TargetDeconstructionRoot,
    TaskDiagnosticInputRevisions, TaskSlots,
};
use hw_logistics::transport_request::{TransportDemand, TransportRequest, WheelbarrowLease};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_logistics::{
    BelongsTo, BucketStorage, Inventory, ResourceItem, SharedResourceCache, Wheelbarrow,
};
use hw_spatial::ResourceSpatialGrid;
use hw_world::WorldMap;
use hw_world::map::WorldMapOwnerSnapshot;

#[derive(Resource, Default)]
pub(crate) struct TaskDiagnosticExternalRevisionState {
    initialized: bool,
    availability_signature: (u64, u64),
    soul_eligibility: HashMap<Entity, SoulEligibilitySnapshot>,
    assigned_move_targets: HashMap<Entity, Entity>,
    move_task_targets: HashMap<Entity, Entity>,
    deconstruction_order_targets: HashMap<Entity, Entity>,
    deconstruction_owner_snapshots: HashMap<Entity, WorldMapOwnerSnapshot>,
    soul_spa_tile_sites: HashMap<Entity, Entity>,
}

#[derive(Debug, Clone, Copy)]
struct SoulEligibilitySnapshot {
    assigned: bool,
    familiar_idle_allowed: bool,
    auto_build_idle_allowed: bool,
    fatigue: f32,
    generate_power_ready: bool,
    has_breakdown: bool,
    commanded_by: Option<Entity>,
}

impl SoulEligibilitySnapshot {
    fn from_components(
        soul: &DamnedSoul,
        assigned_task: &AssignedTask,
        idle: &IdleState,
        has_breakdown: bool,
        commanded_by: Option<&CommandedBy>,
    ) -> Self {
        Self {
            assigned: !matches!(assigned_task, AssignedTask::None),
            familiar_idle_allowed: idle.behavior != IdleBehavior::ExhaustedGathering,
            auto_build_idle_allowed: !matches!(
                idle.behavior,
                IdleBehavior::ExhaustedGathering
                    | IdleBehavior::Resting
                    | IdleBehavior::GoingToRest
                    | IdleBehavior::Escaping
                    | IdleBehavior::Drifting
            ),
            fatigue: soul.fatigue,
            generate_power_ready: soul.dream >= DREAM_GENERATE_ASSIGN_THRESHOLD,
            has_breakdown,
            commanded_by: commanded_by.map(|owner| owner.0),
        }
    }

    fn semantically_differs(self, current: Self, familiar_thresholds: &[f32]) -> bool {
        self.assigned != current.assigned
            || self.familiar_idle_allowed != current.familiar_idle_allowed
            || self.auto_build_idle_allowed != current.auto_build_idle_allowed
            || self.generate_power_ready != current.generate_power_ready
            || self.has_breakdown != current.has_breakdown
            || self.commanded_by != current.commanded_by
            || familiar_thresholds
                .iter()
                .copied()
                .any(|threshold| (self.fatigue <= threshold) != (current.fatigue <= threshold))
            || (self.fatigue < FATIGUE_THRESHOLD) != (current.fatigue < FATIGUE_THRESHOLD)
    }
}

type ChangedTasksQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<Designation>,
        Or<(
            Changed<Designation>,
            Changed<TaskSlots>,
            Changed<TaskWorkers>,
            Changed<Blueprint>,
            Changed<FloorTileBlueprint>,
            Changed<WallTileBlueprint>,
            Changed<ManagedBy>,
            Changed<TransportRequest>,
            Changed<TransportDemand>,
            Changed<Transform>,
            Changed<DeconstructionOrder>,
            Changed<TargetDeconstructionRoot>,
        )>,
    ),
>;

type ChangedFamiliarsQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Changed<Familiar>,
        Changed<FamiliarOperation>,
        Changed<FamiliarPolicy>,
        Changed<ActiveCommand>,
        Changed<TaskArea>,
        Changed<Commanding>,
        Changed<ManagedTasks>,
    )>,
>;

type ChangedSoulsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static DamnedSoul,
        &'static AssignedTask,
        &'static IdleState,
        Option<&'static StressBreakdown>,
        Option<&'static CommandedBy>,
    ),
    Or<(
        Changed<DamnedSoul>,
        Changed<AssignedTask>,
        Changed<IdleState>,
        Changed<StressBreakdown>,
        Changed<CommandedBy>,
    )>,
>;

type ChangedAvailabilityQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Changed<StoredItems>,
        Changed<IncomingDeliveries>,
        Changed<Inventory>,
        Changed<ResourceItem>,
        Changed<Wheelbarrow>,
        Changed<Stockpile>,
        Changed<MudMixerStorage>,
        Changed<Blueprint>,
        Changed<BucketStorage>,
    )>,
>;

type ChangedAvailabilityRelationsQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Changed<ParkedAt>,
        Changed<PushedBy>,
        Changed<WheelbarrowLease>,
        Changed<TransportDemand>,
        Changed<LoadedItems>,
        Changed<LoadedIn>,
        Changed<StoredIn>,
        Changed<DeliveringTo>,
        Changed<BelongsTo>,
    )>,
>;

type ChangedDeconstructionTargetsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static DeconstructionPending>,
        Option<&'static DeconstructionOrders>,
    ),
    (
        Or<(With<DeconstructionPending>, With<DeconstructionOrders>)>,
        Or<(
            Or<(
                Changed<DeconstructionPending>,
                Changed<DeconstructionOrders>,
                Changed<MovePlanned>,
                Changed<PendingBuildingMove>,
                Changed<Building>,
                Changed<Transform>,
                Changed<SandPile>,
                Changed<BonePile>,
                Changed<TransportRequest>,
            )>,
            Or<(
                Changed<Door>,
                Changed<BridgeMarker>,
                Changed<SoulSpaSite>,
                Changed<PowerConsumer>,
                Changed<PowerGenerator>,
                Changed<GeneratesFor>,
                Changed<ConsumesFrom>,
            )>,
            Or<(
                Changed<Stockpile>,
                Changed<MudMixerStorage>,
                Changed<RestArea>,
                Changed<WheelbarrowParking>,
            )>,
        )>,
    ),
>;

type DeconstructionTargetOrdersQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static DeconstructionPending>,
        Option<&'static DeconstructionOrders>,
    ),
    Or<(With<DeconstructionPending>, With<DeconstructionOrders>)>,
>;

type ChangedSoulSpaTilesQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static SoulSpaTile),
    Or<(
        Changed<SoulSpaTile>,
        Changed<Designation>,
        Changed<TaskSlots>,
        Changed<TaskWorkers>,
        Changed<ChildOf>,
    )>,
>;

#[derive(SystemParam)]
pub struct TaskRevisionDetectors<'w, 's> {
    changed_tasks: ChangedTasksQuery<'w, 's>,
    changed_familiars: ChangedFamiliarsQuery<'w, 's>,
    changed_souls: ChangedSoulsQuery<'w, 's>,
    changed_availability: ChangedAvailabilityQuery<'w, 's>,
    changed_availability_relations: ChangedAvailabilityRelationsQuery<'w, 's>,
    changed_deconstruction_targets: ChangedDeconstructionTargetsQuery<'w, 's>,
    deconstruction_target_orders: DeconstructionTargetOrdersQuery<'w, 's>,
    changed_deconstruction_order_targets: Query<
        'w,
        's,
        (Entity, &'static TargetDeconstructionRoot),
        Changed<TargetDeconstructionRoot>,
    >,
    changed_deconstruction_orders:
        Query<'w, 's, Option<&'static TargetDeconstructionRoot>, Changed<DeconstructionOrder>>,
    changed_move_tasks: Query<'w, 's, (Entity, &'static MovePlantTask), Changed<MovePlantTask>>,
    changed_soul_spa_tiles: ChangedSoulSpaTilesQuery<'w, 's>,
    familiar_operations: Query<'w, 's, &'static FamiliarOperation>,
}

#[derive(SystemParam)]
pub struct TaskRevisionRemovals<'w, 's> {
    designations: RemovedComponents<'w, 's, Designation>,
    task_slots: RemovedComponents<'w, 's, TaskSlots>,
    task_workers: RemovedComponents<'w, 's, TaskWorkers>,
    blueprints: RemovedComponents<'w, 's, Blueprint>,
    floor_tiles: RemovedComponents<'w, 's, FloorTileBlueprint>,
    wall_tiles: RemovedComponents<'w, 's, WallTileBlueprint>,
    managed_by: RemovedComponents<'w, 's, ManagedBy>,
    transport_requests: RemovedComponents<'w, 's, TransportRequest>,
    familiars: RemovedComponents<'w, 's, Familiar>,
    familiar_operations: RemovedComponents<'w, 's, FamiliarOperation>,
    familiar_policies: RemovedComponents<'w, 's, FamiliarPolicy>,
    active_commands: RemovedComponents<'w, 's, ActiveCommand>,
    task_areas: RemovedComponents<'w, 's, TaskArea>,
    commandings: RemovedComponents<'w, 's, Commanding>,
    managed_tasks: RemovedComponents<'w, 's, ManagedTasks>,
    damned_souls: RemovedComponents<'w, 's, DamnedSoul>,
    assigned_tasks: RemovedComponents<'w, 's, AssignedTask>,
    idle_states: RemovedComponents<'w, 's, IdleState>,
    stress_breakdowns: RemovedComponents<'w, 's, StressBreakdown>,
    commanded_by: RemovedComponents<'w, 's, CommandedBy>,
    stored_items: RemovedComponents<'w, 's, StoredItems>,
    incoming_deliveries: RemovedComponents<'w, 's, IncomingDeliveries>,
    inventories: RemovedComponents<'w, 's, Inventory>,
    resource_items: RemovedComponents<'w, 's, ResourceItem>,
    wheelbarrows: RemovedComponents<'w, 's, Wheelbarrow>,
    parked_at: RemovedComponents<'w, 's, ParkedAt>,
    pushed_by: RemovedComponents<'w, 's, PushedBy>,
    wheelbarrow_leases: RemovedComponents<'w, 's, WheelbarrowLease>,
    transport_demands: RemovedComponents<'w, 's, TransportDemand>,
    stockpiles: RemovedComponents<'w, 's, Stockpile>,
    mixer_storages: RemovedComponents<'w, 's, MudMixerStorage>,
    bucket_storages: RemovedComponents<'w, 's, BucketStorage>,
    loaded_items: RemovedComponents<'w, 's, LoadedItems>,
    loaded_in: RemovedComponents<'w, 's, LoadedIn>,
    stored_in: RemovedComponents<'w, 's, StoredIn>,
    delivering_to: RemovedComponents<'w, 's, DeliveringTo>,
    belongs_to: RemovedComponents<'w, 's, BelongsTo>,
    deconstruction_orders: RemovedComponents<'w, 's, DeconstructionOrder>,
    deconstruction_order_collections: RemovedComponents<'w, 's, DeconstructionOrders>,
    deconstruction_order_targets: RemovedComponents<'w, 's, TargetDeconstructionRoot>,
    deconstruction_pending: RemovedComponents<'w, 's, DeconstructionPending>,
    move_planned: RemovedComponents<'w, 's, MovePlanned>,
    pending_building_moves: RemovedComponents<'w, 's, PendingBuildingMove>,
    buildings: RemovedComponents<'w, 's, Building>,
    transforms: RemovedComponents<'w, 's, Transform>,
    sand_piles: RemovedComponents<'w, 's, SandPile>,
    bone_piles: RemovedComponents<'w, 's, BonePile>,
    doors: RemovedComponents<'w, 's, Door>,
    bridges: RemovedComponents<'w, 's, BridgeMarker>,
    soul_spa_sites: RemovedComponents<'w, 's, SoulSpaSite>,
    soul_spa_tiles: RemovedComponents<'w, 's, SoulSpaTile>,
    child_of: RemovedComponents<'w, 's, ChildOf>,
    rest_areas: RemovedComponents<'w, 's, RestArea>,
    wheelbarrow_parkings: RemovedComponents<'w, 's, WheelbarrowParking>,
    power_consumers: RemovedComponents<'w, 's, PowerConsumer>,
    power_generators: RemovedComponents<'w, 's, PowerGenerator>,
    generates_for: RemovedComponents<'w, 's, GeneratesFor>,
    consumes_from: RemovedComponents<'w, 's, ConsumesFrom>,
    move_plant_tasks: RemovedComponents<'w, 's, MovePlantTask>,
}

fn bump_deconstruction_orders_for_target(
    targets: &DeconstructionTargetOrdersQuery<'_, '_>,
    revisions: &mut TaskDiagnosticInputRevisions,
    target: Entity,
) {
    let Ok((_, pending, orders)) = targets.get(target) else {
        return;
    };
    if let Some(pending) = pending {
        revisions.bump_task(pending.order);
    }
    if let Some(orders) = orders {
        for &order in orders.iter() {
            revisions.bump_task(order);
        }
    }
}

/// Final semantic revision sync. It runs after auto-gather Commands are
/// applied and immediately before the normal delegation cycle.
pub(crate) fn sync_task_diagnostic_revisions_system(
    detectors: TaskRevisionDetectors,
    mut removed: TaskRevisionRemovals,
    resource_grid: Res<ResourceSpatialGrid>,
    resource_cache: Res<SharedResourceCache>,
    world_map: Res<WorldMap>,
    mut external: ResMut<TaskDiagnosticExternalRevisionState>,
    mut revisions: ResMut<TaskDiagnosticInputRevisions>,
) {
    for entity in &detectors.changed_tasks {
        revisions.bump_task(entity);
    }
    for (_, pending, orders) in &detectors.changed_deconstruction_targets {
        if let Some(pending) = pending {
            revisions.bump_task(pending.order);
        }
        if let Some(orders) = orders {
            for &order in orders.iter() {
                revisions.bump_task(order);
            }
        }
    }
    for (order, relation) in &detectors.changed_deconstruction_order_targets {
        let previous = external
            .deconstruction_order_targets
            .insert(order, relation.0);
        if let Some(previous) = previous.filter(|&previous| previous != relation.0) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                previous,
            );
        }
        bump_deconstruction_orders_for_target(
            &detectors.deconstruction_target_orders,
            &mut revisions,
            relation.0,
        );
    }
    for relation in &detectors.changed_deconstruction_orders {
        let Some(relation) = relation else {
            continue;
        };
        bump_deconstruction_orders_for_target(
            &detectors.deconstruction_target_orders,
            &mut revisions,
            relation.0,
        );
    }
    for (tile, soul_spa_tile) in &detectors.changed_soul_spa_tiles {
        let previous = external
            .soul_spa_tile_sites
            .insert(tile, soul_spa_tile.parent_site);
        if let Some(previous) = previous.filter(|&previous| previous != soul_spa_tile.parent_site) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                previous,
            );
        }
        bump_deconstruction_orders_for_target(
            &detectors.deconstruction_target_orders,
            &mut revisions,
            soul_spa_tile.parent_site,
        );
    }

    let removed_designations: HashSet<_> = removed.designations.read().collect();
    let removed_task_slots: Vec<_> = removed.task_slots.read().collect();
    let removed_task_workers: Vec<_> = removed.task_workers.read().collect();
    let removed_blueprints: Vec<_> = removed.blueprints.read().collect();
    let removed_floor_tiles: Vec<_> = removed.floor_tiles.read().collect();
    let removed_wall_tiles: Vec<_> = removed.wall_tiles.read().collect();
    let removed_managed_by: Vec<_> = removed.managed_by.read().collect();
    let removed_transport_requests: Vec<_> = removed.transport_requests.read().collect();
    let removed_transport_demands: Vec<_> = removed.transport_demands.read().collect();
    let removed_stockpiles: Vec<_> = removed.stockpiles.read().collect();
    let removed_mixer_storages: Vec<_> = removed.mixer_storages.read().collect();
    let removed_rest_areas: Vec<_> = removed.rest_areas.read().collect();
    let removed_wheelbarrow_parkings: Vec<_> = removed.wheelbarrow_parkings.read().collect();
    let removed_child_of: Vec<_> = removed.child_of.read().collect();
    let removed_deconstruction_orders: Vec<_> = removed.deconstruction_orders.read().collect();
    let removed_deconstruction_order_collections: Vec<_> =
        removed.deconstruction_order_collections.read().collect();
    let removed_deconstruction_order_targets: Vec<_> =
        removed.deconstruction_order_targets.read().collect();
    for tile in removed_designations
        .iter()
        .chain(&removed_task_slots)
        .chain(&removed_task_workers)
        .chain(&removed_child_of)
    {
        if let Some(&target) = external.soul_spa_tile_sites.get(tile) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                target,
            );
        }
    }
    for tile in removed.soul_spa_tiles.read() {
        if let Some(target) = external.soul_spa_tile_sites.remove(&tile) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                target,
            );
        }
    }
    for &entity in removed_task_slots
        .iter()
        .chain(&removed_task_workers)
        .chain(&removed_blueprints)
        .chain(&removed_floor_tiles)
        .chain(&removed_wall_tiles)
        .chain(&removed_managed_by)
        .chain(&removed_transport_requests)
        .chain(&removed_transport_demands)
        .chain(&removed_deconstruction_orders)
        .chain(&removed_deconstruction_order_targets)
    {
        if !removed_designations.contains(&entity) {
            revisions.bump_task(entity);
        }
    }
    for entity in removed_designations {
        revisions.remove_task(entity);
    }
    for &order in &removed_deconstruction_orders {
        let Some(&target) = external.deconstruction_order_targets.get(&order) else {
            continue;
        };
        bump_deconstruction_orders_for_target(
            &detectors.deconstruction_target_orders,
            &mut revisions,
            target,
        );
    }
    for &order in &removed_deconstruction_order_targets {
        if detectors
            .changed_deconstruction_order_targets
            .get(order)
            .is_ok()
        {
            continue;
        }
        if let Some(previous) = external.deconstruction_order_targets.remove(&order) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                previous,
            );
        }
    }

    let mut changed_deconstruction_targets = HashSet::new();
    changed_deconstruction_targets.extend(removed_deconstruction_order_collections);
    changed_deconstruction_targets.extend(removed.deconstruction_pending.read());
    changed_deconstruction_targets.extend(removed.move_planned.read());
    changed_deconstruction_targets.extend(removed.pending_building_moves.read());
    changed_deconstruction_targets.extend(removed.buildings.read());
    changed_deconstruction_targets.extend(removed.transforms.read());
    changed_deconstruction_targets.extend(removed.sand_piles.read());
    changed_deconstruction_targets.extend(removed.bone_piles.read());
    changed_deconstruction_targets.extend(removed.doors.read());
    changed_deconstruction_targets.extend(removed.bridges.read());
    changed_deconstruction_targets.extend(removed.soul_spa_sites.read());
    changed_deconstruction_targets.extend(removed_stockpiles.iter().copied());
    changed_deconstruction_targets.extend(removed_mixer_storages.iter().copied());
    changed_deconstruction_targets.extend(removed_rest_areas.iter().copied());
    changed_deconstruction_targets.extend(removed_wheelbarrow_parkings.iter().copied());
    changed_deconstruction_targets.extend(removed.power_consumers.read());
    changed_deconstruction_targets.extend(removed.power_generators.read());
    changed_deconstruction_targets.extend(removed.generates_for.read());
    changed_deconstruction_targets.extend(removed.consumes_from.read());
    for target in changed_deconstruction_targets {
        let Ok((_, pending, orders)) = detectors.deconstruction_target_orders.get(target) else {
            continue;
        };
        if let Some(pending) = pending {
            revisions.bump_task(pending.order);
        }
        if let Some(orders) = orders {
            for &order in orders.iter() {
                revisions.bump_task(order);
            }
        }
    }

    for (move_task, task) in &detectors.changed_move_tasks {
        let previous = external.move_task_targets.insert(move_task, task.building);
        if let Some(previous) = previous.filter(|&previous| previous != task.building) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                previous,
            );
        }
        bump_deconstruction_orders_for_target(
            &detectors.deconstruction_target_orders,
            &mut revisions,
            task.building,
        );
    }
    for move_task in removed.move_plant_tasks.read() {
        if let Some(target) = external.move_task_targets.remove(&move_task) {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                target,
            );
        }
    }

    let mut live_deconstruction_targets = HashSet::new();
    for (target, pending, orders) in &detectors.deconstruction_target_orders {
        if pending.is_none() && orders.is_none() {
            continue;
        }
        live_deconstruction_targets.insert(target);
        let current = world_map.snapshot_owner(target);
        if external
            .deconstruction_owner_snapshots
            .insert(target, current.clone())
            .is_some_and(|previous| previous != current)
        {
            if let Some(pending) = pending {
                revisions.bump_task(pending.order);
            }
            if let Some(orders) = orders {
                for &order in orders.iter() {
                    revisions.bump_task(order);
                }
            }
        }
    }
    external
        .deconstruction_owner_snapshots
        .retain(|target, _| live_deconstruction_targets.contains(target));

    let familiar_thresholds: Vec<_> = detectors
        .familiar_operations
        .iter()
        .map(|operation| operation.fatigue_threshold)
        .collect();
    let mut roster_changed = !detectors.changed_familiars.is_empty();
    for (entity, soul, assigned_task, idle, breakdown, commanded_by) in &detectors.changed_souls {
        let current = SoulEligibilitySnapshot::from_components(
            soul,
            assigned_task,
            idle,
            breakdown.is_some(),
            commanded_by,
        );
        let previous = external.soul_eligibility.insert(entity, current);
        roster_changed |= previous
            .is_none_or(|previous| previous.semantically_differs(current, &familiar_thresholds));

        let current_move_target = match assigned_task {
            AssignedTask::MovePlant(data) => Some(data.building),
            _ => None,
        };
        let previous_move_target = match current_move_target {
            Some(target) => external.assigned_move_targets.insert(entity, target),
            None => external.assigned_move_targets.remove(&entity),
        };
        let affected_move_targets = previous_move_target
            .into_iter()
            .chain(current_move_target)
            .collect::<HashSet<_>>();
        for target in affected_move_targets {
            bump_deconstruction_orders_for_target(
                &detectors.deconstruction_target_orders,
                &mut revisions,
                target,
            );
        }
    }
    roster_changed |= removed.familiars.read().count() > 0;
    roster_changed |= removed.familiar_operations.read().count() > 0;
    roster_changed |= removed.familiar_policies.read().count() > 0;
    roster_changed |= removed.active_commands.read().count() > 0;
    roster_changed |= removed.task_areas.read().count() > 0;
    roster_changed |= removed.commandings.read().count() > 0;
    roster_changed |= removed.managed_tasks.read().count() > 0;
    let mut removed_soul_eligibility = HashSet::new();
    removed_soul_eligibility.extend(removed.damned_souls.read());
    removed_soul_eligibility.extend(removed.assigned_tasks.read());
    removed_soul_eligibility.extend(removed.idle_states.read());
    removed_soul_eligibility.extend(removed.stress_breakdowns.read());
    removed_soul_eligibility.extend(removed.commanded_by.read());
    if !removed_soul_eligibility.is_empty() {
        roster_changed = true;
        for entity in removed_soul_eligibility {
            external.soul_eligibility.remove(&entity);
            if let Some(target) = external.assigned_move_targets.remove(&entity) {
                bump_deconstruction_orders_for_target(
                    &detectors.deconstruction_target_orders,
                    &mut revisions,
                    target,
                );
            }
        }
    }
    if roster_changed {
        revisions.bump_roster();
    }

    let availability_signature = (
        resource_grid.generation(),
        resource_cache.semantic_generation(),
    );
    let mut availability_changed = !detectors.changed_availability.is_empty()
        || !detectors.changed_availability_relations.is_empty();
    availability_changed |= removed.stored_items.read().count() > 0;
    availability_changed |= removed.incoming_deliveries.read().count() > 0;
    availability_changed |= removed.inventories.read().count() > 0;
    availability_changed |= removed.resource_items.read().count() > 0;
    availability_changed |= removed.wheelbarrows.read().count() > 0;
    availability_changed |= removed.parked_at.read().count() > 0;
    availability_changed |= removed.pushed_by.read().count() > 0;
    availability_changed |= removed.wheelbarrow_leases.read().count() > 0;
    availability_changed |= !removed_transport_demands.is_empty();
    availability_changed |= !removed_blueprints.is_empty();
    availability_changed |= !removed_stockpiles.is_empty();
    availability_changed |= !removed_mixer_storages.is_empty();
    availability_changed |= removed.bucket_storages.read().count() > 0;
    availability_changed |= removed.loaded_items.read().count() > 0;
    availability_changed |= removed.loaded_in.read().count() > 0;
    availability_changed |= removed.stored_in.read().count() > 0;
    availability_changed |= removed.delivering_to.read().count() > 0;
    availability_changed |= removed.belongs_to.read().count() > 0;
    availability_changed |=
        external.initialized && availability_signature != external.availability_signature;
    if availability_changed {
        revisions.bump_availability();
    }
    external.availability_signature = availability_signature;
    external.initialized = true;
    if revisions.topology != world_map.obstacle_version {
        revisions.set_topology(world_map.obstacle_version);
    }
}

pub(crate) fn refresh_deconstruction_blockers_after_revision_sync_system(
    revisions: Res<TaskDiagnosticInputRevisions>,
    mut blockers: Query<(Entity, &mut DeconstructionBlocker)>,
) {
    for (order, mut blocker) in &mut blockers {
        if !blocker.active {
            continue;
        }
        match blocker.stamp {
            None => blocker.stamp = Some(revisions.stamp_for(order)),
            Some(stamp) if !revisions.is_current(order, stamp, blocker.domains) => {
                blocker.active = false;
            }
            Some(_) => {}
        }
    }
}

pub(crate) fn reset_task_diagnostics_for_world_replace(world: &mut World) {
    world.insert_resource(TaskDiagnosticInputRevisions::default());
    world.insert_resource(hw_familiar_ai::FamiliarTaskCandidateDiagnostics::default());
    world.insert_resource(hw_soul_ai::BlueprintAutoBuildDiagnostics::default());
    world.insert_resource(TaskDiagnosticExternalRevisionState::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::relationships::WorkingOn;
    use hw_jobs::{DeconstructionBlockReason, TaskDiagnosticDomainMask, WorkType};

    #[test]
    fn world_replace_reset_drops_entity_revisions_and_snapshots() {
        let task = Entity::from_raw_u32(9).expect("valid test entity");
        let mut world = World::new();
        let mut revisions = TaskDiagnosticInputRevisions::default();
        revisions.bump_task(task);
        world.insert_resource(revisions);
        world.insert_resource(hw_familiar_ai::FamiliarTaskCandidateDiagnostics::default());
        world.insert_resource(TaskDiagnosticExternalRevisionState {
            initialized: true,
            availability_signature: (2, 3),
            ..Default::default()
        });

        reset_task_diagnostics_for_world_replace(&mut world);

        assert_eq!(
            world
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(task),
            0
        );
        assert!(
            world
                .resource::<hw_familiar_ai::FamiliarTaskCandidateDiagnostics>()
                .header()
                .is_none()
        );
        assert!(
            !world
                .resource::<TaskDiagnosticExternalRevisionState>()
                .initialized
        );
    }

    #[test]
    fn pending_blocker_arms_after_worker_cleanup_revision_then_wakes_on_target_change() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(
                Update,
                (
                    sync_task_diagnostic_revisions_system,
                    refresh_deconstruction_blockers_after_revision_sync_system,
                )
                    .chain(),
            );

        let target = app.world_mut().spawn_empty().id();
        let order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        let worker = app.world_mut().spawn(WorkingOn(order)).id();
        app.world_mut().flush();
        app.update();

        // Simulate the finalizer, which runs after this frame's revision sync:
        // worker cleanup and the pending blocker become visible together on
        // the next update.
        app.world_mut().entity_mut(worker).remove::<WorkingOn>();
        app.world_mut()
            .entity_mut(order)
            .insert(DeconstructionBlocker::pending(
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK,
            ));
        app.world_mut().flush();
        app.update();

        let armed = *app
            .world()
            .get::<DeconstructionBlocker>(order)
            .expect("blocker remains on durable order");
        assert!(armed.active);
        assert!(armed.stamp.is_some());
        app.update();
        assert_eq!(
            app.world().get::<DeconstructionBlocker>(order),
            Some(&armed),
            "unrelated frame churn must not wake a current blocker"
        );

        let move_task = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(target).insert(MovePlanned {
            task_entity: move_task,
        });
        app.update();

        assert!(
            app.world()
                .get::<DeconstructionBlocker>(order)
                .is_some_and(|blocker| !blocker.active),
            "selected task-domain input change must wake the order"
        );
    }

    #[test]
    fn armed_task_blocker_ignores_unselected_revision_domains() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .add_systems(
                Update,
                refresh_deconstruction_blockers_after_revision_sync_system,
            );
        let order = app
            .world_mut()
            .spawn(DeconstructionBlocker::pending(
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK,
            ))
            .id();

        app.update();
        assert!(
            app.world()
                .get::<DeconstructionBlocker>(order)
                .is_some_and(|blocker| blocker.active && blocker.stamp.is_some())
        );

        app.world_mut()
            .resource_mut::<TaskDiagnosticInputRevisions>()
            .bump_availability();
        app.update();
        assert!(
            app.world()
                .get::<DeconstructionBlocker>(order)
                .is_some_and(|blocker| blocker.active),
            "an unselected availability revision must not wake a task-only blocker"
        );

        app.world_mut()
            .resource_mut::<TaskDiagnosticInputRevisions>()
            .bump_task(order);
        app.update();
        assert!(
            app.world()
                .get::<DeconstructionBlocker>(order)
                .is_some_and(|blocker| !blocker.active),
            "the selected task revision must wake the blocker"
        );
    }

    #[test]
    fn move_planned_add_and_remove_each_bump_only_the_related_order() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        let unrelated = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: WorkType::Chop,
                },
                TaskSlots::new(1),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        app.update();
        let baseline_order = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        let baseline_unrelated = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(unrelated);

        let move_task = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(target).insert(MovePlanned {
            task_entity: move_task,
        });
        app.update();
        let after_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_add, baseline_order);
        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(unrelated),
            baseline_unrelated
        );

        app.world_mut().entity_mut(target).remove::<MovePlanned>();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(order),
            after_add
        );
        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(unrelated),
            baseline_unrelated
        );
    }

    #[test]
    fn reverse_order_relationship_add_and_remove_wake_the_canonical_order() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let canonical_order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending {
                order: canonical_order,
            });
        app.update();
        let baseline = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(canonical_order);

        let competing_order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.update();
        let after_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(canonical_order);
        assert_ne!(after_add, baseline);

        app.world_mut()
            .entity_mut(competing_order)
            .remove::<TargetDeconstructionRoot>();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(canonical_order),
            after_add
        );
    }

    #[test]
    fn deconstruction_order_marker_add_and_remove_wake_the_canonical_order() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let canonical_order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending {
                order: canonical_order,
            });
        let sibling = app.world_mut().spawn(TargetDeconstructionRoot(target)).id();
        app.update();
        let baseline = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(canonical_order);

        app.world_mut()
            .entity_mut(sibling)
            .insert(DeconstructionOrder);
        app.update();
        let after_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(canonical_order);
        assert_ne!(
            after_add, baseline,
            "repairing an owned sibling marker must wake the canonical order"
        );

        app.world_mut()
            .entity_mut(sibling)
            .remove::<DeconstructionOrder>();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(canonical_order),
            after_add,
            "breaking an owned sibling marker must wake the canonical order"
        );
    }

    #[test]
    fn structure_marker_changes_wake_the_pending_deconstruction_order() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        app.update();
        let mut previous = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);

        app.world_mut().entity_mut(target).insert(Door::default());
        app.update();
        let after_door_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_door_add, previous);
        previous = after_door_add;

        app.world_mut().entity_mut(target).remove::<Door>();
        app.update();
        let after_door_remove = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_door_remove, previous);
        previous = after_door_remove;

        app.world_mut().entity_mut(target).insert(BridgeMarker);
        app.update();
        let after_bridge_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_bridge_add, previous);
        previous = after_bridge_add;

        app.world_mut().entity_mut(target).remove::<BridgeMarker>();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(order),
            previous
        );
    }

    #[test]
    fn soul_spa_tile_task_state_and_removal_wake_the_site_order() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: target,
                grid_pos: (4, 5),
            })
            .id();
        app.update();
        let baseline = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);

        app.world_mut().entity_mut(tile).insert(Designation {
            work_type: WorkType::GeneratePower,
        });
        app.update();
        let after_designation = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_designation, baseline);

        app.world_mut().entity_mut(tile).remove::<Designation>();
        app.update();
        let after_designation_remove = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_designation_remove, after_designation);

        app.world_mut().entity_mut(tile).insert(TaskSlots::new(1));
        app.update();
        let after_slots = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_slots, after_designation_remove);

        app.world_mut().entity_mut(tile).remove::<SoulSpaTile>();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(order),
            after_slots
        );
    }

    #[test]
    fn durable_and_assigned_move_lifecycles_bump_the_related_order() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        app.update();
        let baseline = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);

        let move_task = app
            .world_mut()
            .spawn(MovePlantTask {
                building: target,
                destination_grid: (4, 4),
                destination_pos: Vec2::splat(64.0),
                companion_anchor: None,
            })
            .id();
        app.update();
        let after_durable_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_durable_add, baseline);

        app.world_mut().entity_mut(move_task).despawn();
        app.update();
        let after_durable_remove = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_durable_remove, after_durable_add);

        let assigned_move_task = app.world_mut().spawn_empty().id();
        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::MovePlant(hw_jobs::MovePlantData {
                    task_entity: assigned_move_task,
                    building: target,
                    destination_grid: (5, 5),
                    destination_pos: Vec2::splat(80.0),
                    companion_anchor: None,
                    phase: hw_jobs::MovePlantPhase::GoToBuilding,
                }),
                IdleState::default(),
            ))
            .id();
        app.update();
        let after_assigned_add = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        assert_ne!(after_assigned_add, after_durable_remove);

        *app.world_mut()
            .get_mut::<AssignedTask>(soul)
            .expect("fixture soul keeps an assignment") = AssignedTask::None;
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(order),
            after_assigned_add
        );
    }

    #[test]
    fn world_map_owner_replacement_bumps_order_without_topology_change() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let target = app.world_mut().spawn_empty().id();
        let other = app.world_mut().spawn_empty().id();
        let order = app
            .world_mut()
            .spawn((
                DeconstructionOrder,
                Designation {
                    work_type: WorkType::Deconstruct,
                },
                TaskSlots::new(1),
                TargetDeconstructionRoot(target),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building_occupancy((3, 3), target);
        app.update();
        let baseline_revision = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .task_revision(order);
        let baseline_topology = app.world().resource::<WorldMap>().obstacle_version;

        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building((3, 3), other);
        assert_eq!(
            app.world().resource::<WorldMap>().obstacle_version,
            baseline_topology,
            "owner-only replacement deliberately preserves walkability"
        );
        app.update();

        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(order),
            baseline_revision
        );
    }

    #[test]
    fn designation_removal_does_not_recreate_a_task_revision() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);

        let task = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: hw_jobs::WorkType::Chop,
                },
                TaskSlots::new(1),
            ))
            .id();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(task),
            0
        );

        app.world_mut()
            .entity_mut(task)
            .remove::<(Designation, TaskSlots)>();
        app.update();

        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(task),
            0
        );
    }

    #[test]
    fn roster_revision_tracks_eligibility_boundaries_not_idle_timers() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);

        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::None,
                IdleState::default(),
            ))
            .id();
        app.update();
        let initial_revision = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .roster;

        app.world_mut()
            .entity_mut(soul)
            .get_mut::<IdleState>()
            .expect("idle state exists")
            .idle_timer += 1.0;
        app.update();
        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            initial_revision
        );

        app.world_mut()
            .entity_mut(soul)
            .get_mut::<DamnedSoul>()
            .expect("soul exists")
            .fatigue = 0.4;
        app.update();
        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            initial_revision
        );

        app.world_mut()
            .entity_mut(soul)
            .get_mut::<DamnedSoul>()
            .expect("soul exists")
            .fatigue = 0.9;
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            initial_revision
        );
    }

    #[test]
    fn familiar_policy_change_and_removal_each_advance_roster_revision() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation::default(),
                FamiliarPolicy::default(),
            ))
            .id();
        app.update();
        let initial = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .roster;

        app.world_mut()
            .entity_mut(familiar)
            .get_mut::<FamiliarPolicy>()
            .unwrap()
            .set_all_allowed(false);
        app.update();
        let after_change = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .roster;
        assert_ne!(after_change, initial);

        app.world_mut()
            .entity_mut(familiar)
            .remove::<FamiliarPolicy>();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            after_change
        );
    }
}
