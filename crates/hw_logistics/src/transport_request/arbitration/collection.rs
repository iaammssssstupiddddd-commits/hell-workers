use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::{
    SINGLE_BATCH_WAIT_SECS, TILE_SIZE, WHEELBARROW_ARBITRATION_TOP_K,
    WHEELBARROW_PREFERRED_MIN_BATCH_SIZE,
};
use hw_core::relationships::{IncomingDeliveries, StoredIn, StoredItems};
use hw_jobs::{Blueprint, Designation};

use crate::resource_cache::SharedResourceCache;
use crate::transport_request::{
    ManualTransportRequest, ReceiverPolicyTier, TransportDemand, TransportRequest,
    TransportRequestKind, TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::types::{BelongsTo, ResourceItem, ResourceType};
use crate::zone::{Stockpile, StockpilePolicy};

use super::WheelbarrowArbitrationOutcome;
use super::candidates::{
    FreeItemsQuery, RequestEvalInput, build_free_item_buckets, build_request_eval_context,
    collect_top_k_unreserved_nearest, is_pick_drop_possible, score_candidate,
};
use super::types::{
    BatchCandidate, FreeItemSnapshot, ItemBucketKey, NearbyItem, RequestEvalContext,
};

type CollectCandidatesRequestQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TransportRequest,
        &'static TransportRequestState,
        &'static TransportDemand,
        &'static Transform,
        Option<&'static WheelbarrowLease>,
        Option<&'static WheelbarrowPendingSince>,
        Option<&'static ManualTransportRequest>,
        Option<&'static Designation>,
    ),
>;

pub(super) struct CollectCandidatesQueries<'a> {
    pub q_belongs: &'a Query<'a, 'a, &'static BelongsTo>,
    pub q_stored_in: &'a Query<'a, 'a, &'static StoredIn>,
    pub q_stockpiles: &'a Query<
        'a,
        'a,
        (
            &'static Stockpile,
            Option<&'static StockpilePolicy>,
            Option<&'static StoredItems>,
        ),
    >,
    pub q_blueprints: &'a Query<'a, 'a, &'static Blueprint>,
    pub q_incoming: &'a Query<'a, 'a, &'static IncomingDeliveries>,
    pub q_resource_items: &'a Query<'a, 'a, &'static ResourceItem>,
    pub q_receiver_policy_tiers: &'a Query<'a, 'a, &'static ReceiverPolicyTier>,
}

pub(super) struct CollectCandidatesContext<'a> {
    pub available_wheelbarrows: &'a [(Entity, Vec2)],
    pub stale_cleared_requests: &'a HashSet<Entity>,
    pub cache: &'a SharedResourceCache,
    pub now: f64,
    pub outcomes: &'a mut HashMap<Entity, WheelbarrowArbitrationOutcome>,
}

fn insufficient_source_outcome(
    available_count: usize,
    reserved_count: usize,
    hard_min: usize,
) -> WheelbarrowArbitrationOutcome {
    if available_count.saturating_add(reserved_count) >= hard_min {
        WheelbarrowArbitrationOutcome::SourceReserved
    } else {
        WheelbarrowArbitrationOutcome::NoSourceItems
    }
}

struct SourceCandidateBuckets<'a> {
    free_items: &'a [FreeItemSnapshot],
    by_resource: &'a HashMap<ResourceType, Vec<usize>>,
    by_resource_owner_ground: &'a HashMap<(ResourceType, Option<Entity>), Vec<usize>>,
    cache: &'a SharedResourceCache,
}

#[derive(Clone, Copy)]
struct SourceCandidateSearch {
    request_pos: Vec2,
    search_radius_sq: f32,
    top_k: usize,
}

fn collect_request_source_candidates(
    bucket_key: ItemBucketKey,
    buckets: SourceCandidateBuckets<'_>,
    search: SourceCandidateSearch,
) -> (Vec<NearbyItem>, usize, usize) {
    match bucket_key {
        ItemBucketKey::Resource(resource_type) => {
            let bucket = buckets
                .by_resource
                .get(&resource_type)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let (items, reserved) = collect_top_k_unreserved_nearest(
                bucket,
                buckets.free_items,
                search.request_pos,
                search.search_radius_sq,
                search.top_k,
                buckets.cache,
            );
            (items, reserved, bucket.len())
        }
        ItemBucketKey::ResourceOwnerGround {
            resource_type,
            owner,
        } => {
            let exact_bucket = buckets
                .by_resource_owner_ground
                .get(&(resource_type, owner))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let (exact_items, exact_reserved) = collect_top_k_unreserved_nearest(
                exact_bucket,
                buckets.free_items,
                search.request_pos,
                search.search_radius_sq,
                search.top_k,
                buckets.cache,
            );
            if !exact_items.is_empty() || owner.is_none() {
                return (exact_items, exact_reserved, exact_bucket.len());
            }

            // Owned destinations prefer their own resources, then claim unowned ground items.
            // Items owned by another destination never enter this fallback.
            let unowned_bucket = buckets
                .by_resource_owner_ground
                .get(&(resource_type, None))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let (unowned_items, unowned_reserved) = collect_top_k_unreserved_nearest(
                unowned_bucket,
                buckets.free_items,
                search.request_pos,
                search.search_radius_sq,
                search.top_k,
                buckets.cache,
            );
            (
                unowned_items,
                exact_reserved.saturating_add(unowned_reserved),
                exact_bucket.len().saturating_add(unowned_bucket.len()),
            )
        }
    }
}

pub(super) fn collect_candidates(
    q_requests: &CollectCandidatesRequestQuery,
    q_free_items: &FreeItemsQuery,
    queries: CollectCandidatesQueries<'_>,
    context: CollectCandidatesContext<'_>,
) -> (Vec<(BatchCandidate, f32)>, u32, u32, u32, f64) {
    struct EligibleRequest {
        eval: RequestEvalContext,
        kind: TransportRequestKind,
        stockpile_group: Vec<Entity>,
    }

    let mut candidates: Vec<(BatchCandidate, f32)> = Vec::new();
    let mut eligible_requests = 0u32;
    let mut bucket_items_total = 0u32;
    let mut candidates_after_top_k = 0u32;
    let mut pending_secs_total = 0.0f64;

    if context.available_wheelbarrows.is_empty() {
        return (
            candidates,
            eligible_requests,
            bucket_items_total,
            candidates_after_top_k,
            pending_secs_total,
        );
    }

    let mut eligible = Vec::<EligibleRequest>::new();
    for (
        req_entity,
        req,
        state,
        demand,
        transform,
        lease_opt,
        pending_since_opt,
        manual_opt,
        designation_opt,
    ) in q_requests.iter()
    {
        let effective_lease = if context.stale_cleared_requests.contains(&req_entity) {
            None
        } else {
            lease_opt
        };
        let receiver_policy_tier = queries.q_receiver_policy_tiers.get(req_entity).ok();
        let eval = match build_request_eval_context(
            RequestEvalInput {
                req_entity,
                req,
                state,
                demand,
                transform,
                lease_opt: effective_lease,
                pending_since_opt,
                manual_opt,
                designation_opt,
                receiver_policy_tier,
                now: context.now,
            },
            queries.q_belongs,
            queries.q_stockpiles,
            context.cache,
            queries.q_incoming,
            queries.q_resource_items,
        ) {
            Ok(eval) => eval,
            Err(outcome) => {
                context.outcomes.insert(req_entity, outcome);
                continue;
            }
        };

        eligible.push(EligibleRequest {
            eval,
            kind: req.kind,
            stockpile_group: req.stockpile_group.clone(),
        });
    }
    eligible_requests = eligible.len() as u32;
    if eligible.is_empty() {
        return (
            candidates,
            eligible_requests,
            bucket_items_total,
            candidates_after_top_k,
            pending_secs_total,
        );
    }

    let (free_items, by_resource, by_resource_owner_ground) =
        build_free_item_buckets(q_free_items, queries.q_belongs, queries.q_stored_in);
    let search_radius_sq = (TILE_SIZE * 10.0) * (TILE_SIZE * 10.0);

    for req in eligible {
        let eval = req.eval;
        pending_secs_total += eval.pending_for;
        let (mut nearby_items, mut reserved_in_search, inspected_bucket_items) =
            collect_request_source_candidates(
                eval.bucket_key,
                SourceCandidateBuckets {
                    free_items: &free_items,
                    by_resource: &by_resource,
                    by_resource_owner_ground: &by_resource_owner_ground,
                    cache: context.cache,
                },
                SourceCandidateSearch {
                    request_pos: eval.request_pos,
                    search_radius_sq,
                    top_k: WHEELBARROW_ARBITRATION_TOP_K,
                },
            );
        bucket_items_total = bucket_items_total.saturating_add(inspected_bucket_items as u32);

        if nearby_items.is_empty()
            && ((eval.resource_type.requires_wheelbarrow()
                && req.kind == TransportRequestKind::DeliverToBlueprint)
                || req.kind == TransportRequestKind::DeliverToMixerSolid)
        {
            let bucket = by_resource
                .get(&eval.resource_type)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            (nearby_items, reserved_in_search) = collect_top_k_unreserved_nearest(
                bucket,
                &free_items,
                eval.request_pos,
                f32::INFINITY,
                WHEELBARROW_ARBITRATION_TOP_K,
                context.cache,
            );
        }

        candidates_after_top_k += nearby_items.len() as u32;
        if nearby_items.is_empty() {
            context.outcomes.insert(
                eval.request_entity,
                insufficient_source_outcome(0, reserved_in_search, eval.hard_min),
            );
            continue;
        }

        if is_pick_drop_possible(&eval, &nearby_items, queries.q_blueprints) {
            context.outcomes.insert(
                eval.request_entity,
                WheelbarrowArbitrationOutcome::NotApplicable,
            );
            continue;
        }

        let selected_count = nearby_items.len().min(eval.max_items);
        if selected_count < eval.hard_min {
            context.outcomes.insert(
                eval.request_entity,
                insufficient_source_outcome(selected_count, reserved_in_search, eval.hard_min),
            );
            continue;
        }

        let is_small_batch = eval.resource_type.requires_wheelbarrow()
            && req.kind == TransportRequestKind::DeliverToBlueprint
            && selected_count < WHEELBARROW_PREFERRED_MIN_BATCH_SIZE;
        if is_small_batch && eval.pending_for < SINGLE_BATCH_WAIT_SECS {
            context.outcomes.insert(
                eval.request_entity,
                WheelbarrowArbitrationOutcome::PreferredBatchWaiting,
            );
            continue;
        }

        let mut items = Vec::with_capacity(selected_count);
        let mut source_sum = Vec2::ZERO;
        for candidate in nearby_items.iter().take(selected_count) {
            items.push(candidate.entity);
            source_sum += candidate.pos;
        }
        let source_pos = source_sum / selected_count as f32;

        let min_wb_distance = context
            .available_wheelbarrows
            .iter()
            .map(|(_, wb_pos)| wb_pos.distance(source_pos))
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
            .unwrap_or(f32::MAX);

        let score = score_candidate(
            selected_count as f32,
            eval.priority as f32,
            min_wb_distance,
            eval.pending_for,
            is_small_batch,
        );
        debug!(
            "WB Arbitration: candidate req {:?} score {:.2} (batch={}, priority={}, wb_dist={:.1}, pending_for={:.1}, small_batch={})",
            eval.request_entity,
            score,
            selected_count,
            eval.priority,
            min_wb_distance,
            eval.pending_for,
            is_small_batch
        );

        candidates.push((
            BatchCandidate {
                request_entity: eval.request_entity,
                items,
                source_pos,
                destination: eval.destination,
                group_cells: req.stockpile_group,
                resource_type: eval.resource_type,
                receiver_priority: eval.receiver_priority,
                hard_min: eval.hard_min,
                pending_for: eval.pending_for,
                is_small_batch,
            },
            score,
        ));
    }

    candidates.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| {
                left.request_entity
                    .index_u32()
                    .cmp(&right.request_entity.index_u32())
            })
            .then_with(|| {
                left.request_entity
                    .generation()
                    .to_bits()
                    .cmp(&right.request_entity.generation().to_bits())
            })
    });
    (
        candidates,
        eligible_requests,
        bucket_items_total,
        candidates_after_top_k,
        pending_secs_total,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ResourceType;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    #[test]
    fn partial_batch_blocked_by_reservations_is_contention() {
        assert_eq!(
            insufficient_source_outcome(1, 1, 2),
            WheelbarrowArbitrationOutcome::SourceReserved
        );
        assert_eq!(
            insufficient_source_outcome(1, 0, 2),
            WheelbarrowArbitrationOutcome::NoSourceItems
        );
    }

    #[test]
    fn owned_stockpile_falls_back_only_to_unowned_ground_items() {
        let owner = entity(100);
        let other_owner = entity(101);
        let free_items = vec![
            FreeItemSnapshot {
                entity: entity(1),
                pos: Vec2::ZERO,
                resource_type: ResourceType::Wood,
                owner: None,
                is_ground: true,
            },
            FreeItemSnapshot {
                entity: entity(2),
                pos: Vec2::ZERO,
                resource_type: ResourceType::Wood,
                owner: Some(other_owner),
                is_ground: true,
            },
        ];
        let by_resource = HashMap::from([(ResourceType::Wood, vec![0, 1])]);
        let by_owner = HashMap::from([
            ((ResourceType::Wood, None), vec![0]),
            ((ResourceType::Wood, Some(other_owner)), vec![1]),
        ]);

        let (selected, _, _) = collect_request_source_candidates(
            ItemBucketKey::ResourceOwnerGround {
                resource_type: ResourceType::Wood,
                owner: Some(owner),
            },
            SourceCandidateBuckets {
                free_items: &free_items,
                by_resource: &by_resource,
                by_resource_owner_ground: &by_owner,
                cache: &SharedResourceCache::default(),
            },
            SourceCandidateSearch {
                request_pos: Vec2::ZERO,
                search_radius_sq: f32::INFINITY,
                top_k: 4,
            },
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].entity, entity(1));
    }
}
