//! 候補抽出: バケット構築・Top-K 抽出・pick&drop 除外

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::constants::*;
use crate::relationships::{StoredIn, StoredItems};
use crate::systems::jobs::Blueprint;
use crate::systems::logistics::transport_request::{
    can_complete_pick_drop_to_blueprint, can_complete_pick_drop_to_point, ManualHaulPinnedSource,
    ManualTransportRequest, TransportDemand, TransportRequest, TransportRequestKind,
    TransportRequestState, WheelbarrowDestination, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::systems::logistics::{BelongsTo, ResourceItem, ResourceType, ReservedForTask, Stockpile};
use bevy::prelude::*;

use super::types::{FreeItemSnapshot, HeapEntry, ItemBucketKey, NearbyItem, RequestEvalContext};

pub fn build_free_item_buckets(
    q_free_items: &Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
            Without<ManualHaulPinnedSource>,
        ),
    >,
    q_belongs: &Query<&BelongsTo>,
    q_stored_in: &Query<&StoredIn>,
) -> (
    Vec<FreeItemSnapshot>,
    HashMap<ResourceType, Vec<usize>>,
    HashMap<(ResourceType, Option<Entity>), Vec<usize>>,
) {
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

pub fn build_request_eval_context(
    req_entity: Entity,
    req: &TransportRequest,
    state: &TransportRequestState,
    demand: &TransportDemand,
    transform: &Transform,
    lease_opt: Option<&WheelbarrowLease>,
    pending_since_opt: Option<&WheelbarrowPendingSince>,
    manual_opt: Option<&ManualTransportRequest>,
    now: f64,
    q_belongs: &Query<&BelongsTo>,
    q_stockpiles: &Query<(&Stockpile, Option<&StoredItems>)>,
) -> Option<RequestEvalContext> {
    if manual_opt.is_some() {
        return None;
    }
    let eligible_kind = match req.kind {
        TransportRequestKind::DepositToStockpile => true,
        TransportRequestKind::DeliverToBlueprint | TransportRequestKind::DeliverToMixerSolid => {
            req.resource_type.requires_wheelbarrow()
        }
        _ => false,
    };
    if !eligible_kind {
        return None;
    }
    if *state != TransportRequestState::Pending {
        return None;
    }
    if lease_opt.is_some() {
        return None;
    }
    if !req.resource_type.is_loadable() {
        return None;
    }

    let (destination, max_items, bucket_key) = match req.kind {
        TransportRequestKind::DepositToStockpile => {
            let owner = q_belongs.get(req.anchor).ok().map(|belongs| belongs.0);

            let (dest_stockpile, dest_capacity) = if !req.stockpile_group.is_empty() {
                let mut total_free = 0usize;
                let mut best_cell = req.anchor;
                let mut best_free = 0usize;
                for &cell in &req.stockpile_group {
                    if let Ok((stock, stored_opt)) = q_stockpiles.get(cell) {
                        let type_ok = stock.resource_type.is_none()
                            || stock.resource_type == Some(req.resource_type);
                        if !type_ok {
                            continue;
                        }
                        let current = stored_opt.map(|stored| stored.len()).unwrap_or(0);
                        let free = stock.capacity.saturating_sub(current);
                        total_free += free;
                        if free > best_free {
                            best_free = free;
                            best_cell = cell;
                        }
                    }
                }
                (best_cell, total_free)
            } else if let Ok((stock, stored_opt)) = q_stockpiles.get(req.anchor) {
                let type_ok =
                    stock.resource_type.is_none() || stock.resource_type == Some(req.resource_type);
                if !type_ok {
                    return None;
                }
                let current = stored_opt.map(|stored| stored.len()).unwrap_or(0);
                let free = stock.capacity.saturating_sub(current);
                (req.anchor, free)
            } else {
                return None;
            };

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
        TransportRequestKind::DeliverToMixerSolid => (
            WheelbarrowDestination::Mixer {
                entity: req.anchor,
                resource_type: req.resource_type,
            },
            (demand.remaining() as usize).min(WHEELBARROW_CAPACITY),
            ItemBucketKey::Resource(req.resource_type),
        ),
        _ => return None,
    };

    let hard_min = if req.resource_type.requires_wheelbarrow() {
        1
    } else {
        WHEELBARROW_MIN_BATCH_SIZE
    };
    if max_items < hard_min {
        return None;
    }

    Some(RequestEvalContext {
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
        bucket_key,
    })
}

pub fn collect_top_k_nearest(
    bucket: &[usize],
    free_items: &[FreeItemSnapshot],
    request_pos: Vec2,
    search_radius_sq: f32,
    top_k: usize,
) -> Vec<NearbyItem> {
    if top_k == 0 || bucket.is_empty() {
        return Vec::new();
    }

    let mut heap = BinaryHeap::new();
    for &snapshot_idx in bucket {
        let snapshot = free_items[snapshot_idx];
        let dist_sq = snapshot.pos.distance_squared(request_pos);
        if dist_sq > search_radius_sq {
            continue;
        }

        if heap.len() < top_k {
            heap.push(HeapEntry {
                snapshot_idx,
                dist_sq,
            });
            continue;
        }

        if heap
            .peek()
            .is_some_and(|farthest| dist_sq < farthest.dist_sq)
        {
            heap.pop();
            heap.push(HeapEntry {
                snapshot_idx,
                dist_sq,
            });
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
    nearby_items.sort_by(|a, b| a.dist_sq.partial_cmp(&b.dist_sq).unwrap_or(Ordering::Equal));
    nearby_items
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
        WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Mixer { .. } => {
            nearby_items
                .iter()
                .any(|candidate| can_complete_pick_drop_to_point(candidate.pos, eval.request_pos))
        }
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
    is_small_batch: bool,
) -> f32 {
    let mut score = batch_size * WHEELBARROW_SCORE_BATCH_SIZE
        + priority * WHEELBARROW_SCORE_PRIORITY
        - wb_distance * WHEELBARROW_SCORE_DISTANCE;

    if is_small_batch {
        score -= WHEELBARROW_SCORE_SMALL_BATCH_PENALTY;
    }

    score
}
