//! Stockpile consolidation producer.

use std::collections::HashMap;

use bevy::prelude::*;
use hw_core::relationships::{IncomingDeliveries, StoredIn, StoredItems, TaskWorkers};
use hw_jobs::WorkType;

use crate::SharedResourceCache;
use crate::stockpile_policy::{
    StockpilePolicyInput, StockpileTransferPhase, evaluate_stockpile_policy,
};
use crate::transport_request::producer::active_unit_cache::CachedStockpileGroups;
use crate::transport_request::{
    ManualTransportRequest, TransportPriority, TransportRequest, TransportRequestKind,
};
use crate::types::{BelongsTo, ResourceItem, ResourceType};
use crate::zone::{Stockpile, StockpilePolicy};

#[derive(Clone)]
struct CellInfo {
    entity: Entity,
    pos: Vec2,
    stockpile: Stockpile,
    policy: StockpilePolicy,
    stored: usize,
    available_sources: usize,
    incoming_reserved: usize,
    incoming_by_resource: HashMap<ResourceType, usize>,
}

impl CellInfo {
    fn incoming_matching(&self, resource_type: ResourceType) -> usize {
        self.incoming_by_resource
            .get(&resource_type)
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Clone)]
struct DesiredConsolidationRequest {
    issued_by: Entity,
    donor_cells: Vec<Entity>,
    new_assignable: usize,
    pos: Vec2,
    receiver_priority: TransportPriority,
}

struct ConsolidationTransfer {
    receiver: Entity,
    donor_cells: Vec<Entity>,
    amount: usize,
    pos: Vec2,
    receiver_priority: TransportPriority,
}

type ConsolidationStockpileQuery<'w, 's> = Query<
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

type ConsolidationResourceQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static ResourceItem, Option<&'static StoredIn>)>;

type ExistingConsolidationRequestQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TransportRequest,
        Option<&'static TaskWorkers>,
        super::upsert::ExistingStockpileRequestRuntime<'static>,
    ),
    Without<ManualTransportRequest>,
>;

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

fn receiver_available(
    cell: &CellInfo,
    resource_type: ResourceType,
    cycle_reservations: &HashMap<Entity, HashMap<ResourceType, usize>>,
) -> usize {
    let cycle = cycle_reservations.get(&cell.entity);
    let cycle_reserved = cycle.map_or(0, |counts| counts.values().copied().sum());
    let cycle_matching = cycle
        .and_then(|counts| counts.get(&resource_type))
        .copied()
        .unwrap_or(0);
    evaluate_stockpile_policy(StockpilePolicyInput {
        phase: StockpileTransferPhase::NewInbound,
        policy: cell.policy,
        capacity: cell.stockpile.capacity,
        stored_amount: cell.stored,
        stored_resource: cell.stockpile.resource_type,
        transfer_resource: resource_type,
        requested_amount: 0,
        incoming_reserved: cell.incoming_reserved,
        incoming_reserved_other_resource: cell
            .incoming_reserved
            .saturating_sub(cell.incoming_matching(resource_type)),
        cycle_reserved,
        cycle_reserved_other_resource: cycle_reserved.saturating_sub(cycle_matching),
    })
    .available_amount
}

fn donor_available(cell: &CellInfo, resource_type: ResourceType) -> usize {
    evaluate_stockpile_policy(StockpilePolicyInput {
        phase: StockpileTransferPhase::NewOutbound,
        policy: cell.policy,
        capacity: cell.stockpile.capacity,
        stored_amount: cell.stored,
        stored_resource: cell.stockpile.resource_type,
        transfer_resource: resource_type,
        requested_amount: cell.available_sources,
        incoming_reserved: 0,
        incoming_reserved_other_resource: 0,
        cycle_reserved: 0,
        cycle_reserved_other_resource: 0,
    })
    .allowed_amount
}

fn choose_transfer(
    cells: &[&CellInfo],
    resource_type: ResourceType,
    cycle_reservations: &HashMap<Entity, HashMap<ResourceType, usize>>,
) -> Option<ConsolidationTransfer> {
    let mut receivers: Vec<(&CellInfo, usize)> = cells
        .iter()
        .filter_map(|cell| {
            let available = receiver_available(cell, resource_type, cycle_reservations);
            (available > 0).then_some((*cell, available))
        })
        .collect();
    receivers.sort_unstable_by(|(left, _), (right, _)| {
        right
            .stored
            .cmp(&left.stored)
            .then_with(|| left.pos.x.total_cmp(&right.pos.x))
            .then_with(|| left.pos.y.total_cmp(&right.pos.y))
            .then_with(|| entity_sort_key(left.entity).cmp(&entity_sort_key(right.entity)))
    });

    for (receiver, receiver_available) in receivers {
        let mut donors: Vec<&CellInfo> = cells
            .iter()
            .copied()
            .filter(|cell| cell.entity != receiver.entity)
            .filter(|cell| {
                let available = donor_available(cell, resource_type);
                available > 0 && available == cell.stored
            })
            .collect();
        donors.sort_unstable_by(|left, right| {
            left.stored
                .cmp(&right.stored)
                .then_with(|| left.pos.x.total_cmp(&right.pos.x))
                .then_with(|| left.pos.y.total_cmp(&right.pos.y))
                .then_with(|| entity_sort_key(left.entity).cmp(&entity_sort_key(right.entity)))
        });

        let Some(smallest_donor) = donors.first() else {
            continue;
        };
        if receiver_available < smallest_donor.stored {
            continue;
        }
        let donor_total = donors.iter().map(|cell| cell.stored).sum::<usize>();
        let amount = receiver_available.min(donor_total);
        if amount == 0 {
            continue;
        }

        return Some(ConsolidationTransfer {
            receiver: receiver.entity,
            donor_cells: donors.iter().map(|cell| cell.entity).collect(),
            amount,
            pos: receiver.pos,
            receiver_priority: receiver.policy.inbound_priority,
        });
    }

    None
}

pub fn stockpile_consolidation_producer_system(
    mut commands: Commands,
    stockpile_groups_cache: Res<CachedStockpileGroups>,
    resource_cache: Res<SharedResourceCache>,
    q_stockpiles: ConsolidationStockpileQuery,
    q_resource_items: ConsolidationResourceQuery,
    q_existing_requests: ExistingConsolidationRequestQuery,
) {
    let mut available_sources = HashMap::<(Entity, ResourceType), usize>::new();
    for (entity, resource, stored_in) in q_resource_items.iter() {
        let Some(stored_in) = stored_in else {
            continue;
        };
        if resource_cache.get_source_reservation(entity) == 0 {
            *available_sources
                .entry((stored_in.0, resource.0))
                .or_insert(0) += 1;
        }
    }

    let mut group_indices: Vec<usize> = (0..stockpile_groups_cache.groups.len()).collect();
    group_indices.sort_unstable_by(|left, right| {
        let left = &stockpile_groups_cache.groups[*left];
        let right = &stockpile_groups_cache.groups[*right];
        let left_pos = q_stockpiles
            .get(left.representative)
            .map(|(_, transform, ..)| transform.translation.truncate())
            .unwrap_or(Vec2::ZERO);
        let right_pos = q_stockpiles
            .get(right.representative)
            .map(|(_, transform, ..)| transform.translation.truncate())
            .unwrap_or(Vec2::ZERO);
        entity_sort_key(left.owner_yard)
            .cmp(&entity_sort_key(right.owner_yard))
            .then_with(|| left_pos.x.total_cmp(&right_pos.x))
            .then_with(|| left_pos.y.total_cmp(&right_pos.y))
            .then_with(|| {
                entity_sort_key(left.representative).cmp(&entity_sort_key(right.representative))
            })
    });

    let mut desired_requests =
        HashMap::<(Entity, ResourceType), DesiredConsolidationRequest>::new();
    let mut receiver_cycle_reservations = HashMap::<Entity, HashMap<ResourceType, usize>>::new();

    for group_idx in group_indices {
        let group = &stockpile_groups_cache.groups[group_idx];
        let mut cells = Vec::<CellInfo>::new();
        for &cell in &group.cells {
            let Ok((entity, transform, stockpile, policy, stored, incoming, owner)) =
                q_stockpiles.get(cell)
            else {
                continue;
            };
            if owner.map(|owner| owner.0) != Some(group.owner_yard) {
                continue;
            }

            let mut incoming_by_resource = HashMap::new();
            if let Some(incoming) = incoming {
                for item in incoming.iter() {
                    let Ok((_, resource, _)) = q_resource_items.get(*item) else {
                        continue;
                    };
                    *incoming_by_resource.entry(resource.0).or_insert(0) += 1;
                }
            }
            let cell_resource = stockpile.resource_type;
            cells.push(CellInfo {
                entity,
                pos: transform.translation.truncate(),
                stockpile: *stockpile,
                policy: *policy,
                stored: stored.map_or(0, StoredItems::len),
                available_sources: cell_resource
                    .and_then(|resource| available_sources.get(&(entity, resource)).copied())
                    .unwrap_or(0),
                incoming_reserved: incoming.map_or(0, IncomingDeliveries::len),
                incoming_by_resource,
            });
        }

        let mut by_type = HashMap::<ResourceType, Vec<&CellInfo>>::new();
        for cell in &cells {
            if let Some(resource_type) = cell.stockpile.resource_type
                && cell.stored > 0
            {
                by_type.entry(resource_type).or_default().push(cell);
            }
        }
        let mut resource_types: Vec<ResourceType> = by_type.keys().copied().collect();
        resource_types.sort_unstable_by_key(|resource_type| resource_sort_key(*resource_type));

        for resource_type in resource_types {
            let Some(type_cells) = by_type.get(&resource_type) else {
                continue;
            };
            if type_cells.len() < 2 {
                continue;
            }
            let Some(transfer) =
                choose_transfer(type_cells, resource_type, &receiver_cycle_reservations)
            else {
                continue;
            };
            let key = (transfer.receiver, resource_type);
            if desired_requests.contains_key(&key) {
                continue;
            }

            receiver_cycle_reservations
                .entry(transfer.receiver)
                .or_default()
                .entry(resource_type)
                .and_modify(|amount| *amount = amount.saturating_add(transfer.amount))
                .or_insert(transfer.amount);
            desired_requests.insert(
                key,
                DesiredConsolidationRequest {
                    issued_by: group.owner_yard,
                    donor_cells: transfer.donor_cells,
                    new_assignable: transfer.amount,
                    pos: transfer.pos,
                    receiver_priority: transfer.receiver_priority,
                },
            );
        }
    }

    let mut canonical = HashMap::<(Entity, ResourceType), (Entity, usize)>::new();
    for (entity, request, workers, _) in q_existing_requests.iter() {
        if request.kind != TransportRequestKind::ConsolidateStockpile {
            continue;
        }
        let workers = workers.map_or(0, TaskWorkers::len);
        canonical
            .entry((request.anchor, request.resource_type))
            .and_modify(|current| {
                *current = super::upsert::prefer_canonical_request((entity, workers), *current)
            })
            .or_insert((entity, workers));
    }

    for (entity, request, workers, current) in q_existing_requests.iter() {
        if request.kind != TransportRequestKind::ConsolidateStockpile {
            continue;
        }
        let key = (request.anchor, request.resource_type);
        let workers = workers.map_or(0, TaskWorkers::len);
        let is_canonical = canonical.get(&key).is_some_and(|(kept, _)| *kept == entity);
        let desired =
            desired_requests
                .get(&key)
                .map(|desired| super::upsert::StockpileRequestSpec {
                    name: "TransportRequest::ConsolidateStockpile",
                    anchor: key.0,
                    resource_type: key.1,
                    site_pos: desired.pos,
                    issued_by: desired.issued_by,
                    new_assignable: desired.new_assignable,
                    job_priority: 0,
                    transport_priority: TransportPriority::Low,
                    stockpile_group: &desired.donor_cells,
                    receiver_policy_tier: desired.receiver_priority,
                    kind: TransportRequestKind::ConsolidateStockpile,
                    work_type: WorkType::Haul,
                });
        super::upsert::reconcile_stockpile_request(
            &mut commands,
            entity,
            request,
            current,
            workers,
            is_canonical,
            desired.as_ref(),
        );
    }

    let mut missing_requests: Vec<_> = desired_requests
        .into_iter()
        .filter(|(key, _)| !canonical.contains_key(key))
        .collect();
    missing_requests.sort_unstable_by(|(left_key, left), (right_key, right)| {
        left.pos
            .x
            .total_cmp(&right.pos.x)
            .then_with(|| left.pos.y.total_cmp(&right.pos.y))
            .then_with(|| resource_sort_key(left_key.1).cmp(&resource_sort_key(right_key.1)))
            .then_with(|| entity_sort_key(left_key.0).cmp(&entity_sort_key(right_key.0)))
    });
    for (key, desired) in missing_requests {
        super::upsert::spawn_stockpile_request(
            &mut commands,
            super::upsert::StockpileRequestSpec {
                name: "TransportRequest::ConsolidateStockpile",
                anchor: key.0,
                resource_type: key.1,
                site_pos: desired.pos,
                issued_by: desired.issued_by,
                new_assignable: desired.new_assignable,
                job_priority: 0,
                transport_priority: TransportPriority::Low,
                stockpile_group: &desired.donor_cells,
                receiver_policy_tier: desired.receiver_priority,
                kind: TransportRequestKind::ConsolidateStockpile,
                work_type: WorkType::Haul,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_request::producer::stockpile_group::StockpileGroup;
    use crate::transport_request::{ReceiverPolicyTier, TransportDemand, TransportRequestState};
    use crate::zone::StockpileAcceptance;
    use hw_core::relationships::WorkingOn;
    use hw_jobs::{Priority, TaskSlots};

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    fn cell(index: u32, stored: usize, policy: StockpilePolicy, incoming: usize) -> CellInfo {
        CellInfo {
            entity: entity(index),
            pos: Vec2::new(index as f32, 0.0),
            stockpile: Stockpile {
                capacity: 10,
                resource_type: Some(ResourceType::Wood),
            },
            policy,
            stored,
            available_sources: stored,
            incoming_reserved: incoming,
            incoming_by_resource: HashMap::from([(ResourceType::Wood, incoming)]),
        }
    }

    fn producer_app(receiver_priority: TransportPriority) -> (App, Entity, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<SharedResourceCache>()
            .add_systems(Update, stockpile_consolidation_producer_system);
        let owner = app.world_mut().spawn_empty().id();
        let receiver = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 0.0, 0.0),
                Stockpile {
                    capacity: 10,
                    resource_type: Some(ResourceType::Wood),
                },
                StockpilePolicy {
                    inbound_priority: receiver_priority,
                    ..StockpilePolicy::for_capacity(10)
                },
                BelongsTo(owner),
            ))
            .id();
        let donor = app
            .world_mut()
            .spawn((
                Transform::from_xyz(2.0, 0.0, 0.0),
                Stockpile {
                    capacity: 10,
                    resource_type: Some(ResourceType::Wood),
                },
                StockpilePolicy::for_capacity(10),
                BelongsTo(owner),
            ))
            .id();
        for _ in 0..8 {
            app.world_mut()
                .spawn((ResourceItem(ResourceType::Wood), StoredIn(receiver)));
        }
        app.world_mut()
            .spawn((ResourceItem(ResourceType::Wood), StoredIn(donor)));
        let mut groups = CachedStockpileGroups::default();
        groups.groups.push(StockpileGroup {
            cells: vec![receiver, donor],
            owner_yard: owner,
            representative: receiver,
        });
        app.insert_resource(groups);
        (app, owner, receiver, donor)
    }

    fn consolidation_request(world: &mut World, receiver: Entity) -> Entity {
        let mut requests = world.query::<(Entity, &TransportRequest)>();
        requests
            .iter(world)
            .find_map(|(entity, request)| {
                (request.kind == TransportRequestKind::ConsolidateStockpile
                    && request.anchor == receiver)
                    .then_some(entity)
            })
            .expect("consolidation request")
    }

    #[test]
    fn accepted_inventory_respects_export_disabled() {
        let donor = cell(
            2,
            2,
            StockpilePolicy {
                allow_export: false,
                ..StockpilePolicy::for_capacity(10)
            },
            0,
        );

        assert_eq!(donor_available(&donor, ResourceType::Wood), 0);
    }

    #[test]
    fn draining_inventory_can_export_even_when_export_is_disabled() {
        let receiver = cell(1, 8, StockpilePolicy::for_capacity(10), 0);
        let donor = cell(
            2,
            2,
            StockpilePolicy {
                acceptance: StockpileAcceptance::Only(ResourceType::Rock),
                allow_export: false,
                ..StockpilePolicy::for_capacity(10)
            },
            0,
        );

        let transfer = choose_transfer(&[&receiver, &donor], ResourceType::Wood, &HashMap::new())
            .expect("draining donor should be available");

        assert_eq!(transfer.receiver, receiver.entity);
        assert_eq!(transfer.donor_cells, vec![donor.entity]);
        assert_eq!(transfer.amount, 2);
    }

    #[test]
    fn search_continues_when_the_most_filled_receiver_has_no_exportable_donor() {
        let exportable = cell(1, 8, StockpilePolicy::for_capacity(10), 0);
        let receiver_only = cell(
            2,
            1,
            StockpilePolicy {
                allow_export: false,
                ..StockpilePolicy::for_capacity(10)
            },
            0,
        );

        let transfer = choose_transfer(
            &[&exportable, &receiver_only],
            ResourceType::Wood,
            &HashMap::new(),
        )
        .expect("the second receiver can drain the first cell");

        assert_eq!(transfer.receiver, receiver_only.entity);
        assert_eq!(transfer.donor_cells, vec![exportable.entity]);
    }

    #[test]
    fn receiver_target_and_incoming_clamp_new_assignments() {
        let receiver = cell(
            1,
            8,
            StockpilePolicy {
                target_amount: 10,
                inbound_priority: TransportPriority::High,
                ..StockpilePolicy::for_capacity(10)
            },
            1,
        );
        let donor = cell(2, 1, StockpilePolicy::for_capacity(10), 0);

        let transfer = choose_transfer(&[&receiver, &donor], ResourceType::Wood, &HashMap::new())
            .expect("one target slot remains");

        assert_eq!(transfer.amount, 1);
        assert_eq!(transfer.receiver_priority, TransportPriority::High);
    }

    #[test]
    fn receiver_rejects_an_unaccepted_resource() {
        let receiver = cell(
            1,
            8,
            StockpilePolicy {
                acceptance: StockpileAcceptance::Only(ResourceType::Rock),
                ..StockpilePolicy::for_capacity(10)
            },
            0,
        );

        assert_eq!(
            receiver_available(&receiver, ResourceType::Wood, &HashMap::new()),
            0
        );
    }

    #[test]
    fn default_request_keeps_low_base_and_receiver_tier_updates_independently() {
        let (mut app, _, receiver, _) = producer_app(TransportPriority::Normal);

        app.update();
        let request_entity = consolidation_request(app.world_mut(), receiver);
        let request = app
            .world()
            .get::<TransportRequest>(request_entity)
            .expect("transport request");
        assert_eq!(request.priority, TransportPriority::Low);
        assert_eq!(
            app.world()
                .get::<Priority>(request_entity)
                .map(|value| value.0),
            Some(0)
        );
        assert_eq!(
            app.world()
                .get::<ReceiverPolicyTier>(request_entity)
                .map(|tier| tier.0),
            Some(TransportPriority::Normal)
        );

        app.world_mut()
            .get_mut::<StockpilePolicy>(receiver)
            .expect("receiver policy")
            .inbound_priority = TransportPriority::Critical;
        app.update();

        let request = app
            .world()
            .get::<TransportRequest>(request_entity)
            .expect("transport request");
        assert_eq!(request.priority, TransportPriority::Low);
        assert_eq!(
            app.world()
                .get::<ReceiverPolicyTier>(request_entity)
                .map(|tier| tier.0),
            Some(TransportPriority::Critical)
        );
    }

    #[test]
    fn invalidated_request_with_worker_is_capped_to_committed_only() {
        let (mut app, _, receiver, _) = producer_app(TransportPriority::Normal);
        app.update();
        let request_entity = consolidation_request(app.world_mut(), receiver);
        app.world_mut().spawn(WorkingOn(request_entity));
        *app.world_mut()
            .get_mut::<StockpilePolicy>(receiver)
            .expect("receiver policy") = StockpilePolicy {
            acceptance: StockpileAcceptance::Only(ResourceType::Rock),
            inbound_priority: TransportPriority::Normal,
            target_amount: 0,
            allow_export: false,
        };

        app.update();

        let slots = app
            .world()
            .get::<TaskSlots>(request_entity)
            .expect("committed slots");
        let demand = app
            .world()
            .get::<TransportDemand>(request_entity)
            .expect("committed demand");
        assert_eq!(slots.max, 1);
        assert_eq!(demand.desired_slots, 1);
        assert_eq!(demand.inflight, 1);
        assert_eq!(demand.remaining(), 0);
        assert_eq!(
            app.world().get::<TransportRequestState>(request_entity),
            Some(&TransportRequestState::Claimed)
        );
    }
}
