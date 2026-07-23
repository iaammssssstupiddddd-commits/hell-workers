//! 候補抽出: バケット構築・Top-K 抽出・pick&drop 除外

use std::collections::{BinaryHeap, HashMap};

use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::relationships::{IncomingDeliveries, StoredIn, StoredItems};
use hw_jobs::{Blueprint, Designation};

use crate::resource_cache::SharedResourceCache;
use crate::stockpile_policy::{
    StockpilePolicyInput, StockpilePolicyRejection, StockpileTransferPhase,
    evaluate_stockpile_policy,
};
use crate::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, ReceiverPolicyTier, TransportDemand,
    TransportPriority, TransportRequest, TransportRequestKind, TransportRequestState,
    WheelbarrowDestination, WheelbarrowLease, WheelbarrowPendingSince,
    can_complete_pick_drop_to_blueprint, can_complete_pick_drop_to_point,
};
use crate::types::{BelongsTo, ResourceItem, ResourceType};
use crate::zone::{Stockpile, StockpilePolicy};

use super::types::{FreeItemSnapshot, HeapEntry, ItemBucketKey, NearbyItem, RequestEvalContext};
use super::{WheelbarrowArbitrationOutcome, is_wheelbarrow_arbitration_applicable};

pub type FreeItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static ResourceItem,
    ),
    (
        Without<Designation>,
        Without<hw_core::relationships::TaskWorkers>,
        Without<ManualHaulPinnedSource>,
    ),
>;

type FreeItemBuckets = (
    Vec<FreeItemSnapshot>,
    HashMap<ResourceType, Vec<usize>>,
    HashMap<(ResourceType, Option<Entity>), Vec<usize>>,
);

pub fn build_free_item_buckets(
    q_free_items: &FreeItemsQuery,
    q_belongs: &Query<&BelongsTo>,
    q_stored_in: &Query<&StoredIn>,
) -> FreeItemBuckets {
    let mut snapshots = Vec::new();
    let mut by_resource = HashMap::new();
    let mut by_resource_owner_ground = HashMap::new();

    for (entity, transform, visibility, resource_item) in q_free_items.iter() {
        if *visibility == Visibility::Hidden {
            continue;
        }

        let owner = q_belongs.get(entity).ok().map(|belongs| belongs.0);
        let is_ground = q_stored_in.get(entity).is_err();

        let snapshot_idx = snapshots.len();
        let snapshot = FreeItemSnapshot {
            entity,
            pos: transform.translation.truncate(),
            resource_type: resource_item.0,
            owner,
            is_ground,
        };
        snapshots.push(snapshot);

        by_resource
            .entry(snapshot.resource_type)
            .or_insert_with(Vec::new)
            .push(snapshot_idx);
        if snapshot.is_ground {
            by_resource_owner_ground
                .entry((snapshot.resource_type, snapshot.owner))
                .or_insert_with(Vec::new)
                .push(snapshot_idx);
        }
    }

    (snapshots, by_resource, by_resource_owner_ground)
}

/// `build_request_eval_context` に渡すリクエスト単体のデータ。
pub struct RequestEvalInput<'a> {
    pub req_entity: Entity,
    pub req: &'a TransportRequest,
    pub state: &'a TransportRequestState,
    pub demand: &'a TransportDemand,
    pub transform: &'a Transform,
    pub lease_opt: Option<&'a WheelbarrowLease>,
    pub pending_since_opt: Option<&'a WheelbarrowPendingSince>,
    pub manual_opt: Option<&'a ManualTransportRequest>,
    pub designation_opt: Option<&'a Designation>,
    pub receiver_policy_tier: Option<&'a ReceiverPolicyTier>,
    pub now: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StockpileCellAvailability {
    pub available: usize,
    pub blocked_by_reservation: bool,
}

pub(super) fn evaluate_stockpile_cell(
    cell: Entity,
    resource_type: ResourceType,
    receiver_priority: TransportPriority,
    q_stockpiles: &Query<(&Stockpile, Option<&StockpilePolicy>, Option<&StoredItems>)>,
    q_incoming: &Query<&IncomingDeliveries>,
    q_resource_items: &Query<&ResourceItem>,
    cycle_reserved_by_resource: Option<&HashMap<ResourceType, usize>>,
) -> Option<StockpileCellAvailability> {
    let (stockpile, policy, stored) = q_stockpiles.get(cell).ok()?;
    let stored_amount = stored.map_or(0, StoredItems::len);
    let incoming = q_incoming.get(cell).ok();
    let incoming_reserved = incoming.map_or(0, IncomingDeliveries::len);
    let incoming_matching = incoming.map_or(0, |incoming| {
        incoming
            .iter()
            .filter(|item| {
                q_resource_items
                    .get(**item)
                    .is_ok_and(|item| item.0 == resource_type)
            })
            .count()
    });
    let incoming_other = incoming_reserved.saturating_sub(incoming_matching);
    let cycle_reserved = cycle_reserved_by_resource.map_or(0, |counts| counts.values().sum());
    let cycle_matching = cycle_reserved_by_resource
        .and_then(|counts| counts.get(&resource_type))
        .copied()
        .unwrap_or(0);
    let cycle_other = cycle_reserved.saturating_sub(cycle_matching);

    if let Some(policy) = policy {
        if policy.inbound_priority != receiver_priority {
            return Some(StockpileCellAvailability {
                available: 0,
                blocked_by_reservation: false,
            });
        }
        let evaluation = evaluate_stockpile_policy(StockpilePolicyInput {
            phase: StockpileTransferPhase::NewInbound,
            policy: *policy,
            capacity: stockpile.capacity,
            stored_amount,
            stored_resource: stockpile.resource_type,
            transfer_resource: resource_type,
            requested_amount: 0,
            incoming_reserved,
            incoming_reserved_other_resource: incoming_other,
            cycle_reserved,
            cycle_reserved_other_resource: cycle_other,
        });
        return Some(StockpileCellAvailability {
            available: evaluation.available_amount,
            blocked_by_reservation: matches!(
                evaluation.rejection,
                Some(
                    StockpilePolicyRejection::ReservedCapacityReached
                        | StockpilePolicyRejection::ReservedResourceMismatch
                )
            ),
        });
    }

    let type_ok =
        stockpile.resource_type.is_none() || stockpile.resource_type == Some(resource_type);
    if !type_ok {
        return Some(StockpileCellAvailability {
            available: 0,
            blocked_by_reservation: false,
        });
    }
    let physical_remaining = stockpile.capacity.saturating_sub(stored_amount);
    let available = physical_remaining
        .saturating_sub(incoming_reserved)
        .saturating_sub(cycle_reserved);
    Some(StockpileCellAvailability {
        available,
        blocked_by_reservation: physical_remaining > 0 && available == 0,
    })
}

pub fn build_request_eval_context(
    input: RequestEvalInput<'_>,
    q_belongs: &Query<&BelongsTo>,
    q_stockpiles: &Query<(&Stockpile, Option<&StockpilePolicy>, Option<&StoredItems>)>,
    _cache: &SharedResourceCache,
    q_incoming: &Query<&hw_core::relationships::IncomingDeliveries>,
    q_resource_items: &Query<&ResourceItem>,
) -> Result<RequestEvalContext, WheelbarrowArbitrationOutcome> {
    let RequestEvalInput {
        req_entity,
        req,
        state,
        demand,
        transform,
        lease_opt,
        pending_since_opt,
        manual_opt,
        designation_opt,
        receiver_policy_tier,
        now,
    } = input;
    if manual_opt.is_some() {
        return Err(WheelbarrowArbitrationOutcome::NotApplicable);
    }
    if !is_wheelbarrow_arbitration_applicable(req) {
        return Err(WheelbarrowArbitrationOutcome::NotApplicable);
    }
    if *state != TransportRequestState::Pending {
        return Err(WheelbarrowArbitrationOutcome::NotApplicable);
    }
    if designation_opt.is_none() || demand.remaining() == 0 {
        return Err(WheelbarrowArbitrationOutcome::DemandGone);
    }
    if lease_opt.is_some() {
        return Err(WheelbarrowArbitrationOutcome::LeaseGranted);
    }
    if !req.resource_type.is_loadable() {
        return Err(WheelbarrowArbitrationOutcome::NotApplicable);
    }

    let (destination, max_items, bucket_key) = match req.kind {
        TransportRequestKind::DepositToStockpile => {
            let receiver_priority = receiver_policy_tier.map_or(req.priority, |tier| tier.0);
            let cells = if req.stockpile_group.is_empty() {
                std::slice::from_ref(&req.anchor)
            } else {
                req.stockpile_group.as_slice()
            };
            let mut best = None::<(Entity, usize)>;
            let mut blocked_by_reservation = false;
            let mut saw_cell = false;
            for &cell in cells {
                let Some(evaluation) = evaluate_stockpile_cell(
                    cell,
                    req.resource_type,
                    receiver_priority,
                    q_stockpiles,
                    q_incoming,
                    q_resource_items,
                    None,
                ) else {
                    continue;
                };
                saw_cell = true;
                blocked_by_reservation |= evaluation.blocked_by_reservation;
                if evaluation.available > best.map_or(0, |(_, available)| available) {
                    best = Some((cell, evaluation.available));
                }
            }
            let Some((dest_stockpile, dest_capacity)) =
                best.filter(|(_, available)| *available > 0)
            else {
                return Err(if !saw_cell {
                    WheelbarrowArbitrationOutcome::StaleInput
                } else if blocked_by_reservation {
                    WheelbarrowArbitrationOutcome::CapacityReserved
                } else {
                    WheelbarrowArbitrationOutcome::NoDestinationCapacity
                });
            };
            let owner = q_belongs.get(dest_stockpile).ok().map(|belongs| belongs.0);

            (
                WheelbarrowDestination::Stockpile(dest_stockpile),
                dest_capacity.min(WHEELBARROW_CAPACITY),
                ItemBucketKey::ResourceOwnerGround {
                    resource_type: req.resource_type,
                    owner,
                },
            )
        }
        TransportRequestKind::DeliverToBlueprint => (
            WheelbarrowDestination::Blueprint(req.anchor),
            (demand.remaining() as usize).min(WHEELBARROW_CAPACITY),
            ItemBucketKey::Resource(req.resource_type),
        ),
        TransportRequestKind::DeliverToFloorConstruction => (
            WheelbarrowDestination::Blueprint(req.anchor),
            (demand.remaining() as usize).min(WHEELBARROW_CAPACITY),
            ItemBucketKey::Resource(req.resource_type),
        ),
        TransportRequestKind::DeliverToMixerSolid => (
            WheelbarrowDestination::Mixer {
                entity: req.anchor,
                resource_type: req.resource_type,
            },
            (demand.remaining() as usize).min(WHEELBARROW_CAPACITY),
            ItemBucketKey::Resource(req.resource_type),
        ),
        _ => return Err(WheelbarrowArbitrationOutcome::NotApplicable),
    };

    let hard_min = if req.resource_type.requires_wheelbarrow()
        && matches!(
            req.kind,
            TransportRequestKind::DeliverToBlueprint | TransportRequestKind::DeliverToMixerSolid
        ) {
        1
    } else {
        WHEELBARROW_MIN_BATCH_SIZE
    };
    if max_items < hard_min {
        return Err(match req.kind {
            TransportRequestKind::DepositToStockpile => {
                WheelbarrowArbitrationOutcome::NoDestinationCapacity
            }
            _ => WheelbarrowArbitrationOutcome::DemandGone,
        });
    }

    Ok(RequestEvalContext {
        request_entity: req_entity,
        request_pos: transform.translation.truncate(),
        resource_type: req.resource_type,
        destination,
        max_items,
        hard_min,
        pending_for: pending_since_opt
            .map(|pending| now - pending.0)
            .unwrap_or(0.0),
        priority: req.priority as u32,
        receiver_priority: receiver_policy_tier.map_or(req.priority, |tier| tier.0),
        bucket_key,
    })
}

pub fn collect_top_k_unreserved_nearest(
    bucket: &[usize],
    free_items: &[FreeItemSnapshot],
    request_pos: Vec2,
    search_radius_sq: f32,
    top_k: usize,
    cache: &SharedResourceCache,
) -> (Vec<NearbyItem>, usize) {
    if top_k == 0 || bucket.is_empty() {
        return (Vec::new(), 0);
    }

    let mut heap = BinaryHeap::new();
    let mut reserved_in_range = 0usize;
    for &snapshot_idx in bucket {
        let snapshot = free_items[snapshot_idx];
        let dist_sq = snapshot.pos.distance_squared(request_pos);
        if dist_sq > search_radius_sq {
            continue;
        }
        if cache.get_source_reservation(snapshot.entity) > 0 {
            reserved_in_range = reserved_in_range.saturating_add(1);
            continue;
        }

        let entry = HeapEntry {
            snapshot_idx,
            dist_sq,
            entity_index: snapshot.entity.index_u32(),
            entity_generation: snapshot.entity.generation().to_bits(),
        };
        if heap.len() < top_k {
            heap.push(entry);
            continue;
        }

        if heap.peek().is_some_and(|farthest| entry < *farthest) {
            heap.pop();
            heap.push(entry);
        }
    }

    let mut nearby_items: Vec<NearbyItem> = heap
        .into_iter()
        .map(|entry| {
            let snapshot = free_items[entry.snapshot_idx];
            NearbyItem {
                entity: snapshot.entity,
                pos: snapshot.pos,
                dist_sq: entry.dist_sq,
            }
        })
        .collect();
    nearby_items.sort_by(|a, b| {
        a.dist_sq
            .total_cmp(&b.dist_sq)
            .then_with(|| a.entity.index_u32().cmp(&b.entity.index_u32()))
            .then_with(|| {
                a.entity
                    .generation()
                    .to_bits()
                    .cmp(&b.entity.generation().to_bits())
            })
    });
    (nearby_items, reserved_in_range)
}

pub fn is_pick_drop_possible(
    eval: &RequestEvalContext,
    nearby_items: &[NearbyItem],
    q_blueprints: &Query<&Blueprint>,
) -> bool {
    if !eval.resource_type.requires_wheelbarrow() {
        return false;
    }
    match eval.destination {
        WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Mixer { .. } => nearby_items
            .iter()
            .any(|candidate| can_complete_pick_drop_to_point(candidate.pos, eval.request_pos)),
        WheelbarrowDestination::Blueprint(blueprint_entity) => {
            q_blueprints.get(blueprint_entity).ok().is_some_and(|bp| {
                nearby_items.iter().any(|candidate| {
                    can_complete_pick_drop_to_blueprint(candidate.pos, &bp.occupied_grids)
                })
            })
        }
    }
}

pub fn score_candidate(
    batch_size: f32,
    priority: f32,
    wb_distance: f32,
    pending_for: f64,
    is_small_batch: bool,
) -> f32 {
    let mut score = batch_size * WHEELBARROW_SCORE_BATCH_SIZE
        + priority * WHEELBARROW_SCORE_PRIORITY
        - wb_distance * WHEELBARROW_SCORE_DISTANCE;
    let pending_bonus_secs = pending_for.min(WHEELBARROW_SCORE_PENDING_TIME_MAX_SECS);
    score += pending_bonus_secs as f32 * WHEELBARROW_SCORE_PENDING_TIME;

    if is_small_batch {
        score -= WHEELBARROW_SCORE_SMALL_BATCH_PENALTY;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid test entity")
    }

    #[test]
    fn nearest_selection_skips_reserved_items_before_top_k() {
        let reserved = entity(1);
        let available = entity(2);
        let far_reserved = entity(3);
        let snapshots = vec![
            FreeItemSnapshot {
                entity: reserved,
                pos: Vec2::new(1.0, 0.0),
                resource_type: ResourceType::Wood,
                owner: None,
                is_ground: true,
            },
            FreeItemSnapshot {
                entity: available,
                pos: Vec2::new(2.0, 0.0),
                resource_type: ResourceType::Wood,
                owner: None,
                is_ground: true,
            },
            FreeItemSnapshot {
                entity: far_reserved,
                pos: Vec2::new(100.0, 0.0),
                resource_type: ResourceType::Wood,
                owner: None,
                is_ground: true,
            },
        ];
        let mut cache = SharedResourceCache::default();
        cache.reserve_source(reserved, 1);
        cache.reserve_source(far_reserved, 1);

        let (selected, reserved_count) =
            collect_top_k_unreserved_nearest(&[0, 1, 2], &snapshots, Vec2::ZERO, 25.0, 1, &cache);

        assert_eq!(reserved_count, 1);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].entity, available);
    }

    #[test]
    fn reservations_outside_the_actual_search_range_do_not_change_the_outcome() {
        let far_reserved = entity(3);
        let snapshots = vec![FreeItemSnapshot {
            entity: far_reserved,
            pos: Vec2::new(100.0, 0.0),
            resource_type: ResourceType::Wood,
            owner: None,
            is_ground: true,
        }];
        let mut cache = SharedResourceCache::default();
        cache.reserve_source(far_reserved, 1);

        let (selected, reserved_count) =
            collect_top_k_unreserved_nearest(&[0], &snapshots, Vec2::ZERO, 25.0, 1, &cache);

        assert!(selected.is_empty());
        assert_eq!(reserved_count, 0);
    }

    #[test]
    fn equal_distance_top_k_uses_entity_key_instead_of_query_order() {
        let snapshots = vec![
            FreeItemSnapshot {
                entity: entity(9),
                pos: Vec2::X,
                resource_type: ResourceType::Wood,
                owner: None,
                is_ground: true,
            },
            FreeItemSnapshot {
                entity: entity(2),
                pos: -Vec2::X,
                resource_type: ResourceType::Wood,
                owner: None,
                is_ground: true,
            },
        ];

        let (selected, _) = collect_top_k_unreserved_nearest(
            &[0, 1],
            &snapshots,
            Vec2::ZERO,
            4.0,
            1,
            &SharedResourceCache::default(),
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].entity, entity(2));
    }

    #[test]
    fn wheelbarrow_candidate_score_is_monotonic_across_receiver_tiers() {
        let scores = [
            TransportPriority::Low,
            TransportPriority::Normal,
            TransportPriority::High,
            TransportPriority::Critical,
        ]
        .map(|priority| score_candidate(3.0, priority as u32 as f32, 1.0, 0.0, false));

        assert!(scores.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
