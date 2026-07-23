//! Task area auto-haul system

use std::collections::HashMap;
use std::time::Instant;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::{IncomingDeliveries, ManagedBy, StoredItems, TaskWorkers};
use hw_jobs::{Designation, Priority, TaskSlots, WorkType};

use crate::stockpile_policy::{
    StockpilePolicyInput, StockpileTransferPhase, evaluate_stockpile_policy,
    stockpile_owner_accepts_item,
};
use crate::transport_request::producer::active_unit_cache::CachedStockpileGroups;
use crate::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, ReceiverPolicyTier, TransportDemand,
    TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestMetrics, TransportRequestState,
};
use crate::types::{BelongsTo, ResourceItem, ResourceType};
use crate::zone::{Stockpile, StockpilePolicy};

use super::stockpile_group::{
    StockpileGroup, StockpileGroupSpatialIndex, find_nearest_group_for_item_indexed,
};

const PRIORITIES_DESCENDING: [TransportPriority; 4] = [
    TransportPriority::Critical,
    TransportPriority::High,
    TransportPriority::Normal,
    TransportPriority::Low,
];

type StockpilesDetailQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Stockpile,
        &'static StockpilePolicy,
        Option<&'static StoredItems>,
        Option<&'static IncomingDeliveries>,
        Option<&'static BelongsTo>,
    ),
>;

type FreeItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static ResourceItem,
        &'static Visibility,
        Option<&'static BelongsTo>,
    ),
    (
        Without<Designation>,
        Without<TaskWorkers>,
        Without<ManualHaulPinnedSource>,
        Without<hw_core::relationships::StoredIn>,
    ),
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StockpileRequestKey {
    owner_yard: Entity,
    resource_type: ResourceType,
    priority: TransportPriority,
}

#[derive(Clone)]
struct StockpileCellSnapshot {
    entity: Entity,
    pos: Vec2,
    stockpile: Stockpile,
    policy: StockpilePolicy,
    stored_amount: usize,
    incoming_total: usize,
    incoming_by_resource: HashMap<ResourceType, usize>,
    owner: Option<Entity>,
}

#[derive(Debug, Clone, Copy)]
struct CycleReservation {
    resource_type: ResourceType,
    amount: usize,
}

struct GroupEvalContext {
    owner_yard: Entity,
    representative: Entity,
    rep_pos: Vec2,
    cells: Vec<StockpileCellSnapshot>,
}

#[derive(Debug, Clone, Copy)]
struct BestResourceCandidate {
    item_entity: Entity,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    dist_sq: f32,
}

struct DesiredStockpileRequest {
    key: StockpileRequestKey,
    anchor: Entity,
    issued_by: Entity,
    pos: Vec2,
    group_cells: Vec<Entity>,
    new_assignable: usize,
}

fn entity_sort_key(entity: Entity) -> (u32, u32) {
    (entity.index_u32(), entity.generation().to_bits())
}

const fn resource_sort_key(resource_type: ResourceType) -> u8 {
    match resource_type {
        ResourceType::Wood => 0,
        ResourceType::Rock => 1,
        ResourceType::Water => 2,
        ResourceType::BucketEmpty => 3,
        ResourceType::BucketWater => 4,
        ResourceType::Sand => 5,
        ResourceType::Bone => 6,
        ResourceType::StasisMud => 7,
        ResourceType::Wheelbarrow => 8,
    }
}

fn compare_request_keys(
    left: StockpileRequestKey,
    right: StockpileRequestKey,
) -> std::cmp::Ordering {
    entity_sort_key(left.owner_yard)
        .cmp(&entity_sort_key(right.owner_yard))
        .then_with(|| left.priority.cmp(&right.priority))
        .then_with(|| {
            resource_sort_key(left.resource_type).cmp(&resource_sort_key(right.resource_type))
        })
}

fn incoming_resource_counts(
    incoming: Option<&IncomingDeliveries>,
    q_resource_items: &Query<&ResourceItem>,
) -> (usize, HashMap<ResourceType, usize>) {
    let Some(incoming) = incoming else {
        return (0, HashMap::new());
    };

    let mut by_resource = HashMap::new();
    for item in incoming.iter() {
        if let Ok(resource) = q_resource_items.get(*item) {
            *by_resource.entry(resource.0).or_insert(0) += 1;
        }
    }
    (incoming.len(), by_resource)
}

fn build_group_eval_contexts(
    groups: &[StockpileGroup],
    q_stockpiles: &StockpilesDetailQuery<'_, '_>,
    q_resource_items: &Query<&ResourceItem>,
) -> Vec<Option<GroupEvalContext>> {
    groups
        .iter()
        .map(|group| {
            let mut cells = Vec::with_capacity(group.cells.len());
            for &cell in &group.cells {
                let Ok((entity, transform, stockpile, policy, stored, incoming, belongs)) =
                    q_stockpiles.get(cell)
                else {
                    continue;
                };
                let (incoming_total, incoming_by_resource) =
                    incoming_resource_counts(incoming, q_resource_items);
                cells.push(StockpileCellSnapshot {
                    entity,
                    pos: transform.translation.truncate(),
                    stockpile: *stockpile,
                    policy: *policy,
                    stored_amount: stored.map_or(0, StoredItems::len),
                    incoming_total,
                    incoming_by_resource,
                    owner: belongs.map(|owner| owner.0),
                });
            }
            if cells.is_empty() {
                return None;
            }

            let rep_pos = cells
                .iter()
                .find(|cell| cell.entity == group.representative)
                .map_or(Vec2::ZERO, |cell| cell.pos);
            Some(GroupEvalContext {
                owner_yard: group.owner_yard,
                representative: group.representative,
                rep_pos,
                cells,
            })
        })
        .collect()
}

fn evaluate_cell(
    cell: &StockpileCellSnapshot,
    resource_type: ResourceType,
    cycle_reservations: &HashMap<Entity, CycleReservation>,
    requested_amount: usize,
) -> crate::stockpile_policy::StockpilePolicyEvaluation {
    let incoming_matching = cell
        .incoming_by_resource
        .get(&resource_type)
        .copied()
        .unwrap_or(0);
    let incoming_other = cell.incoming_total.saturating_sub(incoming_matching);
    let cycle = cycle_reservations.get(&cell.entity).copied();
    let cycle_reserved = cycle.map_or(0, |reservation| reservation.amount);
    let cycle_other = cycle
        .filter(|reservation| reservation.resource_type != resource_type)
        .map_or(0, |reservation| reservation.amount);

    evaluate_stockpile_policy(StockpilePolicyInput {
        phase: StockpileTransferPhase::NewInbound,
        policy: cell.policy,
        capacity: cell.stockpile.capacity,
        stored_amount: cell.stored_amount,
        stored_resource: cell.stockpile.resource_type,
        transfer_resource: resource_type,
        requested_amount,
        incoming_reserved: cell.incoming_total,
        incoming_reserved_other_resource: incoming_other,
        cycle_reserved,
        cycle_reserved_other_resource: cycle_other,
    })
}

fn pick_representative_resource_type_per_tier(
    groups: &[StockpileGroup],
    spatial_index: &StockpileGroupSpatialIndex,
    contexts: &[Option<GroupEvalContext>],
    q_free_items: &FreeItemsQuery<'_, '_>,
) -> (
    HashMap<(usize, TransportPriority), BestResourceCandidate>,
    u32,
    u32,
) {
    let mut best = HashMap::new();
    let mut free_items_scanned = 0u32;
    let mut items_matched = 0u32;
    let empty_shadow = HashMap::new();

    let mut group_lookup = HashMap::<(Entity, Entity), usize>::new();
    for (idx, group) in groups.iter().enumerate() {
        group_lookup.insert((group.representative, group.owner_yard), idx);
    }

    for (item_entity, transform, resource, visibility, belongs) in q_free_items.iter() {
        free_items_scanned = free_items_scanned.saturating_add(1);
        if *visibility == Visibility::Hidden
            || !resource.0.is_loadable()
            || !resource.0.can_store_in_stockpile()
        {
            continue;
        }

        let item_pos = transform.translation.truncate();
        let Some(group) = find_nearest_group_for_item_indexed(item_pos, groups, spatial_index)
        else {
            continue;
        };
        let Some(&group_idx) = group_lookup.get(&(group.representative, group.owner_yard)) else {
            continue;
        };
        let Some(context) = contexts.get(group_idx).and_then(Option::as_ref) else {
            continue;
        };
        let item_owner = belongs.map(|owner| owner.0);
        let dist_sq = item_pos.distance_squared(context.rep_pos);
        let mut matched = false;

        for priority in PRIORITIES_DESCENDING {
            let can_accept = context.cells.iter().any(|cell| {
                cell.policy.inbound_priority == priority
                    && stockpile_owner_accepts_item(item_owner, cell.owner)
                    && evaluate_cell(cell, resource.0, &empty_shadow, 0).available_amount > 0
            });
            if !can_accept {
                continue;
            }
            matched = true;

            let candidate = BestResourceCandidate {
                item_entity,
                resource_type: resource.0,
                item_owner,
                dist_sq,
            };
            best.entry((group_idx, priority))
                .and_modify(|current: &mut BestResourceCandidate| {
                    if candidate.dist_sq < current.dist_sq
                        || (candidate.dist_sq == current.dist_sq
                            && entity_sort_key(candidate.item_entity)
                                < entity_sort_key(current.item_entity))
                    {
                        *current = candidate;
                    }
                })
                .or_insert(candidate);
        }
        if matched {
            items_matched = items_matched.saturating_add(1);
        }
    }

    (best, free_items_scanned, items_matched)
}

fn representative_cell(cells: &[(&StockpileCellSnapshot, usize)]) -> (Entity, Vec2) {
    let centroid = cells.iter().map(|(cell, _)| cell.pos).sum::<Vec2>() / cells.len() as f32;
    cells
        .iter()
        .min_by(|(left, _), (right, _)| {
            left.pos
                .distance_squared(centroid)
                .total_cmp(&right.pos.distance_squared(centroid))
                .then_with(|| left.pos.x.total_cmp(&right.pos.x))
                .then_with(|| left.pos.y.total_cmp(&right.pos.y))
                .then_with(|| entity_sort_key(left.entity).cmp(&entity_sort_key(right.entity)))
        })
        .map(|(cell, _)| (cell.entity, cell.pos))
        .expect("eligible tier has a representative")
}

fn reserve_cycle_capacity(
    reservations: &mut HashMap<Entity, CycleReservation>,
    entity: Entity,
    resource_type: ResourceType,
    amount: usize,
) {
    let entry = reservations.entry(entity).or_insert(CycleReservation {
        resource_type,
        amount: 0,
    });
    debug_assert_eq!(entry.resource_type, resource_type);
    entry.amount = entry.amount.saturating_add(amount);
}

fn allocate_tier_request(
    context: &GroupEvalContext,
    priority: TransportPriority,
    selected: BestResourceCandidate,
    cycle_reservations: &mut HashMap<Entity, CycleReservation>,
) -> Option<DesiredStockpileRequest> {
    let mut eligible: Vec<(&StockpileCellSnapshot, usize)> = context
        .cells
        .iter()
        .filter(|cell| cell.policy.inbound_priority == priority)
        .filter(|cell| stockpile_owner_accepts_item(selected.item_owner, cell.owner))
        .filter_map(|cell| {
            let available =
                evaluate_cell(cell, selected.resource_type, cycle_reservations, 0).available_amount;
            (available > 0).then_some((cell, available))
        })
        .collect();
    if eligible.is_empty() {
        return None;
    }

    eligible.sort_unstable_by(|(left, left_free), (right, right_free)| {
        left_free
            .cmp(right_free)
            .then_with(|| left.pos.x.total_cmp(&right.pos.x))
            .then_with(|| left.pos.y.total_cmp(&right.pos.y))
            .then_with(|| entity_sort_key(left.entity).cmp(&entity_sort_key(right.entity)))
    });
    let (anchor, pos) = representative_cell(&eligible);
    let mut new_assignable = 0usize;
    let mut group_cells = Vec::with_capacity(eligible.len());
    for (cell, available) in eligible {
        reserve_cycle_capacity(
            cycle_reservations,
            cell.entity,
            selected.resource_type,
            available,
        );
        new_assignable = new_assignable.saturating_add(available);
        group_cells.push(cell.entity);
    }

    Some(DesiredStockpileRequest {
        key: StockpileRequestKey {
            owner_yard: context.owner_yard,
            resource_type: selected.resource_type,
            priority,
        },
        anchor,
        issued_by: context.owner_yard,
        pos,
        group_cells,
        new_assignable,
    })
}

fn build_desired_requests(
    contexts: &[Option<GroupEvalContext>],
    selected: &HashMap<(usize, TransportPriority), BestResourceCandidate>,
) -> HashMap<StockpileRequestKey, DesiredStockpileRequest> {
    let mut context_indices: Vec<usize> = contexts
        .iter()
        .enumerate()
        .filter_map(|(idx, context)| context.as_ref().map(|_| idx))
        .collect();
    context_indices.sort_unstable_by(|left, right| {
        let left = contexts[*left].as_ref().expect("filtered context");
        let right = contexts[*right].as_ref().expect("filtered context");
        entity_sort_key(left.owner_yard)
            .cmp(&entity_sort_key(right.owner_yard))
            .then_with(|| {
                entity_sort_key(left.representative).cmp(&entity_sort_key(right.representative))
            })
    });

    let mut desired = HashMap::new();
    let mut cycle_reservations = HashMap::new();
    for group_idx in context_indices {
        let context = contexts[group_idx].as_ref().expect("filtered context");
        for priority in PRIORITIES_DESCENDING {
            let Some(selected) = selected.get(&(group_idx, priority)).copied() else {
                continue;
            };
            if let Some(request) =
                allocate_tier_request(context, priority, selected, &mut cycle_reservations)
            {
                desired.insert(request.key, request);
            }
        }
    }
    desired
}

fn request_key(request: &TransportRequest) -> StockpileRequestKey {
    StockpileRequestKey {
        owner_yard: request.issued_by,
        resource_type: request.resource_type,
        priority: request.priority,
    }
}

fn prefer_canonical(candidate: (Entity, usize), current: (Entity, usize)) -> (Entity, usize) {
    match (candidate.1 > 0, current.1 > 0) {
        (true, false) => candidate,
        (false, true) => current,
        _ if entity_sort_key(candidate.0) < entity_sort_key(current.0) => candidate,
        _ => current,
    }
}

type ExistingRequestRuntime<'a> = (
    Option<&'a Transform>,
    Option<&'a Visibility>,
    Option<&'a Designation>,
    Option<&'a ManagedBy>,
    Option<&'a TaskSlots>,
    Option<&'a Priority>,
    Option<&'a ReceiverPolicyTier>,
    Option<&'a TransportDemand>,
    Option<&'a TransportRequestState>,
    Option<&'a TransportPolicy>,
);

fn request_matches(current: &TransportRequest, desired: &TransportRequest) -> bool {
    current.kind == desired.kind
        && current.anchor == desired.anchor
        && current.resource_type == desired.resource_type
        && current.issued_by == desired.issued_by
        && current.priority == desired.priority
        && current.stockpile_group == desired.stockpile_group
}

fn transport_policy_is_default(policy: &TransportPolicy) -> bool {
    let default = TransportPolicy::default();
    policy.allow_cross_area_source == default.allow_cross_area_source
        && policy.allow_cross_familiar_claim == default.allow_cross_familiar_claim
        && policy.source_search_radius_tiles == default.source_search_radius_tiles
}

fn upsert_active_request(
    commands: &mut Commands,
    entity: Entity,
    current_request: &TransportRequest,
    current: ExistingRequestRuntime<'_>,
    desired: &DesiredStockpileRequest,
    workers: usize,
) {
    let (
        transform,
        visibility,
        designation,
        managed_by,
        slots,
        priority,
        receiver_tier,
        demand,
        state,
        policy,
    ) = current;
    let desired_slots = super::to_u32_saturating(workers.saturating_add(desired.new_assignable));
    let inflight = super::to_u32_saturating(workers);
    let desired_transform = Transform::from_xyz(desired.pos.x, desired.pos.y, 0.0);
    let desired_request = TransportRequest {
        kind: TransportRequestKind::DepositToStockpile,
        anchor: desired.anchor,
        resource_type: desired.key.resource_type,
        issued_by: desired.issued_by,
        priority: desired.key.priority,
        stockpile_group: desired.group_cells.clone(),
    };
    let desired_state = super::upsert::request_state_for_workers(workers);
    let mut entity_commands = commands.entity(entity);

    if transform.is_none_or(|current| {
        current.translation != desired_transform.translation
            || current.rotation != desired_transform.rotation
            || current.scale != desired_transform.scale
    }) {
        entity_commands.try_insert(desired_transform);
    }
    if visibility.is_none_or(|current| *current != Visibility::Hidden) {
        entity_commands.try_insert(Visibility::Hidden);
    }
    if designation.is_none_or(|current| current.work_type != WorkType::Haul) {
        entity_commands.try_insert(Designation {
            work_type: WorkType::Haul,
        });
    }
    if managed_by.is_none_or(|current| current.0 != desired.issued_by) {
        entity_commands.try_insert(ManagedBy(desired.issued_by));
    }
    if slots.is_none_or(|current| current.max != desired_slots) {
        entity_commands.try_insert(TaskSlots::new(desired_slots));
    }
    if priority.is_none_or(|current| current.0 != 0) {
        entity_commands.try_insert(Priority(0));
    }
    if !request_matches(current_request, &desired_request) {
        entity_commands.try_insert(desired_request);
    }
    if receiver_tier.is_none_or(|current| current.0 != desired.key.priority) {
        entity_commands.try_insert(ReceiverPolicyTier(desired.key.priority));
    }
    if demand.is_none_or(|current| {
        current.desired_slots != desired_slots || current.inflight != inflight
    }) {
        entity_commands.try_insert(TransportDemand {
            desired_slots,
            inflight,
        });
    }
    if state.is_none_or(|current| *current != desired_state) {
        entity_commands.try_insert(desired_state);
    }
    if policy.is_none_or(|current| !transport_policy_is_default(current)) {
        entity_commands.try_insert(TransportPolicy::default());
    }
}

fn cap_committed_request(
    commands: &mut Commands,
    entity: Entity,
    workers: usize,
    current: ExistingRequestRuntime<'_>,
) {
    let workers = super::to_u32_saturating(workers);
    let (_, _, _, _, slots, _, _, demand, state, _) = current;
    let mut entity_commands = commands.entity(entity);
    if slots.is_none_or(|current| current.max != workers) {
        entity_commands.try_insert(TaskSlots::new(workers));
    }
    if demand.is_none_or(|current| current.desired_slots != workers || current.inflight != workers)
    {
        entity_commands.try_insert(TransportDemand {
            desired_slots: workers,
            inflight: workers,
        });
    }
    if state.is_none_or(|current| *current != TransportRequestState::Claimed) {
        entity_commands.try_insert(TransportRequestState::Claimed);
    }
}

fn disable_workerless_request(
    commands: &mut Commands,
    entity: Entity,
    current: ExistingRequestRuntime<'_>,
) {
    let (_, _, designation, _, slots, priority, receiver_tier, demand, _, _) = current;
    let mut entity_commands = commands.entity(entity);
    if designation.is_some() {
        entity_commands.try_remove::<Designation>();
    }
    if slots.is_some() {
        entity_commands.try_remove::<TaskSlots>();
    }
    if priority.is_some() {
        entity_commands.try_remove::<Priority>();
    }
    if receiver_tier.is_some() {
        entity_commands.try_remove::<ReceiverPolicyTier>();
    }
    if demand.is_none_or(|current| current.desired_slots != 0 || current.inflight != 0) {
        entity_commands.try_insert(TransportDemand {
            desired_slots: 0,
            inflight: 0,
        });
    }
}

/// `task_area_auto_haul_system` の ECS クエリ・リソースをまとめた SystemParam。
#[derive(SystemParam)]
pub struct TaskAreaAutoHaulParams<'w, 's> {
    pub stockpile_groups_cache: Res<'w, CachedStockpileGroups>,
    pub q_stockpiles: StockpilesDetailQuery<'w, 's>,
    pub q_resource_items: Query<'w, 's, &'static ResourceItem>,
    pub q_stockpile_requests: Query<
        'w,
        's,
        (
            Entity,
            &'static TransportRequest,
            Option<&'static TaskWorkers>,
            ExistingRequestRuntime<'static>,
        ),
        Without<ManualTransportRequest>,
    >,
    pub q_free_items: FreeItemsQuery<'w, 's>,
    pub metrics: ResMut<'w, TransportRequestMetrics>,
}

pub fn task_area_auto_haul_system(mut commands: Commands, mut p: TaskAreaAutoHaulParams) {
    let started_at = Instant::now();
    let groups = &p.stockpile_groups_cache.groups;
    let contexts = build_group_eval_contexts(groups, &p.q_stockpiles, &p.q_resource_items);
    let (selected, free_items_scanned, items_matched) = pick_representative_resource_type_per_tier(
        groups,
        &p.stockpile_groups_cache.spatial_index,
        &contexts,
        &p.q_free_items,
    );
    let desired_requests = build_desired_requests(&contexts, &selected);

    let mut canonical = HashMap::<StockpileRequestKey, (Entity, usize)>::new();
    for (entity, request, workers, _) in p.q_stockpile_requests.iter() {
        if request.kind != TransportRequestKind::DepositToStockpile {
            continue;
        }
        let workers = workers.map_or(0, TaskWorkers::len);
        canonical
            .entry(request_key(request))
            .and_modify(|current| *current = prefer_canonical((entity, workers), *current))
            .or_insert((entity, workers));
    }

    for (entity, request, workers, current) in p.q_stockpile_requests.iter() {
        if request.kind != TransportRequestKind::DepositToStockpile {
            continue;
        }
        let key = request_key(request);
        let workers = workers.map_or(0, TaskWorkers::len);
        let is_canonical = canonical.get(&key).is_some_and(|(kept, _)| *kept == entity);
        if !is_canonical {
            if workers == 0 {
                commands.entity(entity).try_despawn();
            } else {
                cap_committed_request(&mut commands, entity, workers, current);
            }
            continue;
        }

        if let Some(desired) = desired_requests.get(&key) {
            upsert_active_request(&mut commands, entity, request, current, desired, workers);
        } else if workers == 0 {
            disable_workerless_request(&mut commands, entity, current);
        } else {
            cap_committed_request(&mut commands, entity, workers, current);
        }
    }

    let mut missing_requests: Vec<_> = desired_requests
        .into_iter()
        .filter(|(key, _)| !canonical.contains_key(key))
        .collect();
    missing_requests.sort_unstable_by(|(left, _), (right, _)| compare_request_keys(*left, *right));
    for (key, desired) in missing_requests {
        let desired_slots = super::to_u32_saturating(desired.new_assignable);
        commands.spawn((
            Name::new("TransportRequest::DepositToStockpile"),
            Transform::from_xyz(desired.pos.x, desired.pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            hw_core::relationships::ManagedBy(desired.issued_by),
            TaskSlots::new(desired_slots),
            Priority(0),
            TransportRequest {
                kind: TransportRequestKind::DepositToStockpile,
                anchor: desired.anchor,
                resource_type: key.resource_type,
                issued_by: desired.issued_by,
                priority: key.priority,
                stockpile_group: desired.group_cells,
            },
            ReceiverPolicyTier(key.priority),
            TransportDemand {
                desired_slots,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }

    p.metrics.task_area_groups = groups.len() as u32;
    p.metrics.task_area_free_items_scanned = free_items_scanned;
    p.metrics.task_area_items_matched = items_matched;
    p.metrics.task_area_elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SharedResourceCache;
    use crate::transport_request::arbitration::WheelbarrowArbitrationRuntime;
    use crate::transport_request::{
        WheelbarrowArbitrationDiagnostics, wheelbarrow_arbitration_system,
    };
    use crate::zone::StockpileAcceptance;
    use bevy::app::ScheduleRunnerPlugin;
    use hw_world::Yard;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    fn cell(index: u32, priority: TransportPriority, capacity: usize) -> StockpileCellSnapshot {
        StockpileCellSnapshot {
            entity: entity(index),
            pos: Vec2::new(index as f32, 0.0),
            stockpile: Stockpile {
                capacity,
                resource_type: None,
            },
            policy: StockpilePolicy {
                acceptance: StockpileAcceptance::Any,
                inbound_priority: priority,
                target_amount: capacity,
                allow_export: true,
            },
            stored_amount: 0,
            incoming_total: 0,
            incoming_by_resource: HashMap::new(),
            owner: Some(entity(100)),
        }
    }

    fn selected(resource_type: ResourceType) -> BestResourceCandidate {
        BestResourceCandidate {
            item_entity: entity(200),
            resource_type,
            item_owner: None,
            dist_sq: 0.0,
        }
    }

    #[test]
    fn tier_allocation_keeps_high_and_low_capacity_separate() {
        let context = GroupEvalContext {
            owner_yard: entity(100),
            representative: entity(1),
            rep_pos: Vec2::ZERO,
            cells: vec![
                cell(1, TransportPriority::High, 1),
                cell(2, TransportPriority::Low, 3),
            ],
        };
        let mut shadow = HashMap::new();

        let high = allocate_tier_request(
            &context,
            TransportPriority::High,
            selected(ResourceType::Wood),
            &mut shadow,
        )
        .unwrap();
        let low = allocate_tier_request(
            &context,
            TransportPriority::Low,
            selected(ResourceType::Wood),
            &mut shadow,
        )
        .unwrap();

        assert_eq!(high.new_assignable, 1);
        assert_eq!(high.group_cells, vec![entity(1)]);
        assert_eq!(high.key.priority, TransportPriority::High);
        assert_eq!(low.new_assignable, 3);
        assert_eq!(low.group_cells, vec![entity(2)]);
        assert_eq!(low.key.priority, TransportPriority::Low);
    }

    #[test]
    fn overlapping_yards_reserve_a_physical_cell_only_once() {
        let shared_cell = cell(1, TransportPriority::Normal, 1);
        let first = GroupEvalContext {
            owner_yard: entity(100),
            representative: entity(1),
            rep_pos: Vec2::ZERO,
            cells: vec![shared_cell.clone()],
        };
        let second = GroupEvalContext {
            owner_yard: entity(101),
            representative: entity(1),
            rep_pos: Vec2::ZERO,
            cells: vec![shared_cell],
        };
        let mut shadow = HashMap::new();

        assert!(
            allocate_tier_request(
                &first,
                TransportPriority::Normal,
                selected(ResourceType::Wood),
                &mut shadow,
            )
            .is_some()
        );
        assert!(
            allocate_tier_request(
                &second,
                TransportPriority::Normal,
                selected(ResourceType::Rock),
                &mut shadow,
            )
            .is_none()
        );
    }

    #[test]
    fn overlapping_yard_groups_spawn_only_one_cells_worth_of_live_demand() {
        let mut app = App::new();
        app.init_resource::<CachedStockpileGroups>()
            .init_resource::<TransportRequestMetrics>()
            .add_systems(Update, task_area_auto_haul_system);

        let yard_a_bounds = Yard {
            min: Vec2::new(-16.0, -16.0),
            max: Vec2::new(8.0, 16.0),
        };
        let yard_b_bounds = Yard {
            min: Vec2::new(-8.0, -16.0),
            max: Vec2::new(16.0, 16.0),
        };
        let yard_a = app.world_mut().spawn(yard_a_bounds.clone()).id();
        let yard_b = app.world_mut().spawn(yard_b_bounds.clone()).id();
        let stockpile = app
            .world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 1,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(1),
                BelongsTo(yard_a),
            ))
            .id();
        app.world_mut().spawn((
            Transform::from_xyz(-12.0, 0.0, 0.0),
            ResourceItem(ResourceType::Wood),
            Visibility::Visible,
        ));
        app.world_mut().spawn((
            Transform::from_xyz(12.0, 0.0, 0.0),
            ResourceItem(ResourceType::Wood),
            Visibility::Visible,
        ));

        let groups = vec![
            StockpileGroup {
                cells: vec![stockpile],
                owner_yard: yard_a,
                representative: stockpile,
            },
            StockpileGroup {
                cells: vec![stockpile],
                owner_yard: yard_b,
                representative: stockpile,
            },
        ];
        let spatial_index = super::super::stockpile_group::build_group_spatial_index(
            &groups,
            &[(yard_a, yard_a_bounds), (yard_b, yard_b_bounds)],
        );
        {
            let mut cache = app.world_mut().resource_mut::<CachedStockpileGroups>();
            cache.groups = groups;
            cache.spatial_index = spatial_index;
        }

        app.update();

        let mut requests = app
            .world_mut()
            .query::<(&TransportRequest, &TaskSlots, &TransportDemand)>();
        let requests: Vec<_> = requests
            .iter(app.world())
            .filter(|(request, _, _)| request.kind == TransportRequestKind::DepositToStockpile)
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.anchor, stockpile);
        assert_eq!(requests[0].0.stockpile_group, vec![stockpile]);
        assert_eq!(requests[0].1.max, 1);
        assert_eq!(requests[0].2.desired_slots, 1);
        let metrics = app.world().resource::<TransportRequestMetrics>();
        assert_eq!(metrics.task_area_groups, 2);
        assert_eq!(metrics.task_area_items_matched, 2);
    }

    #[test]
    fn reserved_other_resource_blocks_an_empty_any_cell() {
        let mut cell = cell(1, TransportPriority::Normal, 2);
        cell.incoming_total = 1;
        cell.incoming_by_resource.insert(ResourceType::Wood, 1);

        let result = evaluate_cell(&cell, ResourceType::Rock, &HashMap::new(), 0);
        assert_eq!(
            result.rejection,
            Some(crate::stockpile_policy::StockpilePolicyRejection::ReservedResourceMismatch)
        );
    }

    #[test]
    fn committed_request_is_the_deterministic_canonical_entity() {
        let workerless = (entity(1), 0);
        let committed = (entity(9), 1);
        assert_eq!(prefer_canonical(committed, workerless), committed);
        assert_eq!(prefer_canonical(workerless, committed), committed);
        assert_eq!(prefer_canonical((entity(2), 0), workerless), workerless);
    }

    #[test]
    fn producer_spawns_distinct_live_requests_for_each_priority_tier() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.init_resource::<CachedStockpileGroups>()
            .init_resource::<TransportRequestMetrics>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationRuntime>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .add_systems(
                Update,
                (
                    task_area_auto_haul_system,
                    ApplyDeferred,
                    wheelbarrow_arbitration_system,
                )
                    .chain(),
            );

        let yard_bounds = Yard {
            min: Vec2::splat(-16.0),
            max: Vec2::splat(16.0),
        };
        let yard = app.world_mut().spawn(yard_bounds.clone()).id();
        let high = app
            .world_mut()
            .spawn((
                Transform::from_xyz(-1.0, 0.0, 0.0),
                Stockpile {
                    capacity: 1,
                    resource_type: None,
                },
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Any,
                    inbound_priority: TransportPriority::High,
                    target_amount: 1,
                    allow_export: true,
                },
                BelongsTo(yard),
            ))
            .id();
        let low = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 0.0, 0.0),
                Stockpile {
                    capacity: 3,
                    resource_type: None,
                },
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Any,
                    inbound_priority: TransportPriority::Low,
                    target_amount: 3,
                    allow_export: true,
                },
                BelongsTo(yard),
            ))
            .id();
        app.world_mut().spawn((
            Transform::default(),
            ResourceItem(ResourceType::Wood),
            Visibility::Inherited,
        ));

        let groups = vec![StockpileGroup {
            cells: vec![high, low],
            owner_yard: yard,
            representative: high,
        }];
        let spatial_index = super::super::stockpile_group::build_group_spatial_index(
            &groups,
            &[(yard, yard_bounds)],
        );
        {
            let mut cache = app.world_mut().resource_mut::<CachedStockpileGroups>();
            cache.groups = groups;
            cache.spatial_index = spatial_index;
        }

        app.update();

        let mut requests = app.world_mut().query::<(
            &TransportRequest,
            &ReceiverPolicyTier,
            &TaskSlots,
            &TransportDemand,
        )>();
        let by_priority: HashMap<TransportPriority, (Entity, Vec<Entity>, u32, u32)> = requests
            .iter(app.world())
            .map(|(request, tier, slots, demand)| {
                assert_eq!(request.priority, tier.0);
                (
                    request.priority,
                    (
                        request.anchor,
                        request.stockpile_group.clone(),
                        slots.max,
                        demand.remaining(),
                    ),
                )
            })
            .collect();

        assert_eq!(by_priority.len(), 2);
        assert_eq!(
            by_priority.get(&TransportPriority::High),
            Some(&(high, vec![high], 1, 1))
        );
        assert_eq!(
            by_priority.get(&TransportPriority::Low),
            Some(&(low, vec![low], 3, 3))
        );

        let request_entities: HashMap<TransportPriority, Entity> = app
            .world_mut()
            .query::<(Entity, &TransportRequest)>()
            .iter(app.world())
            .map(|(entity, request)| (request.priority, entity))
            .collect();
        assert!(
            entity_sort_key(request_entities[&TransportPriority::Low])
                < entity_sort_key(request_entities[&TransportPriority::High]),
            "stable key order must determine request Entity allocation"
        );

        // The first arbitration pass creates `WheelbarrowPendingSince`; let that one-time
        // lifecycle write settle before measuring producer steady state.
        app.update();
        let settled_generation = app
            .world()
            .resource::<WheelbarrowArbitrationDiagnostics>()
            .header()
            .expect("settled arbitration pass publishes diagnostics")
            .generation;
        app.update();
        let steady_generation = app
            .world()
            .resource::<WheelbarrowArbitrationDiagnostics>()
            .header()
            .expect("diagnostics remain available")
            .generation;
        assert_eq!(
            steady_generation, settled_generation,
            "an unchanged producer tick must not dirty arbitration inputs"
        );
    }
}
