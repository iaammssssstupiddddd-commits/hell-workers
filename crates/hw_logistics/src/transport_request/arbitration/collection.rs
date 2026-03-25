use std::cmp::Ordering;
use std::collections::HashSet;

use bevy::prelude::*;
use hw_core::constants::{
    SINGLE_BATCH_WAIT_SECS, TILE_SIZE, WHEELBARROW_ARBITRATION_TOP_K,
    WHEELBARROW_PREFERRED_MIN_BATCH_SIZE,
};
use hw_core::relationships::{IncomingDeliveries, StoredIn, StoredItems};
use hw_jobs::Blueprint;

use crate::resource_cache::SharedResourceCache;
use crate::transport_request::{
    ManualTransportRequest, TransportDemand, TransportRequest,
    TransportRequestKind, TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::types::BelongsTo;
use crate::zone::Stockpile;

use super::candidates::{
    FreeItemsQuery, RequestEvalInput, build_free_item_buckets, build_request_eval_context,
    collect_top_k_nearest, is_pick_drop_possible, score_candidate,
};
use super::types::{BatchCandidate, ItemBucketKey, RequestEvalContext};

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
    ),
>;

pub(super) struct CollectCandidatesQueries<'a> {
    pub q_belongs: &'a Query<'a, 'a, &'static BelongsTo>,
    pub q_stored_in: &'a Query<'a, 'a, &'static StoredIn>,
    pub q_stockpiles: &'a Query<'a, 'a, (&'static Stockpile, Option<&'static StoredItems>)>,
    pub q_blueprints: &'a Query<'a, 'a, &'static Blueprint>,
    pub q_incoming: &'a Query<'a, 'a, &'static IncomingDeliveries>,
}

pub(super) fn collect_candidates(
    q_requests: &CollectCandidatesRequestQuery,
    q_free_items: &FreeItemsQuery,
    queries: CollectCandidatesQueries<'_>,
    available_wheelbarrows: &[(Entity, Vec2)],
    stale_cleared_requests: &HashSet<Entity>,
    cache: &SharedResourceCache,
    now: f64,
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

    if available_wheelbarrows.is_empty() {
        return (
            candidates,
            eligible_requests,
            bucket_items_total,
            candidates_after_top_k,
            pending_secs_total,
        );
    }

    let mut eligible = Vec::<EligibleRequest>::new();
    for (req_entity, req, state, demand, transform, lease_opt, pending_since_opt, manual_opt) in
        q_requests.iter()
    {
        let effective_lease = if stale_cleared_requests.contains(&req_entity) {
            None
        } else {
            lease_opt
        };
        let Some(eval) = build_request_eval_context(
            RequestEvalInput {
                req_entity,
                req,
                state,
                demand,
                transform,
                lease_opt: effective_lease,
                pending_since_opt,
                manual_opt,
                now,
            },
            queries.q_belongs,
            queries.q_stockpiles,
            cache,
            queries.q_incoming,
        ) else {
            continue;
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
        let bucket: &[usize] = match eval.bucket_key {
            ItemBucketKey::Resource(resource_type) => by_resource
                .get(&resource_type)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            ItemBucketKey::ResourceOwnerGround {
                resource_type,
                owner,
            } => by_resource_owner_ground
                .get(&(resource_type, owner))
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        };
        bucket_items_total += bucket.len() as u32;

        let mut nearby_items = collect_top_k_nearest(
            bucket,
            &free_items,
            eval.request_pos,
            search_radius_sq,
            WHEELBARROW_ARBITRATION_TOP_K,
        );

        if nearby_items.is_empty()
            && ((eval.resource_type.requires_wheelbarrow()
                && req.kind == TransportRequestKind::DeliverToBlueprint)
                || req.kind == TransportRequestKind::DeliverToMixerSolid)
        {
            nearby_items = collect_top_k_nearest(
                bucket,
                &free_items,
                eval.request_pos,
                f32::INFINITY,
                WHEELBARROW_ARBITRATION_TOP_K,
            );
        }

        candidates_after_top_k += nearby_items.len() as u32;
        if nearby_items.is_empty() {
            continue;
        }

        if is_pick_drop_possible(&eval, &nearby_items, queries.q_blueprints) {
            continue;
        }

        let selected_count = nearby_items.len().min(eval.max_items);
        if selected_count < eval.hard_min {
            continue;
        }

        let is_small_batch = eval.resource_type.requires_wheelbarrow()
            && req.kind == TransportRequestKind::DeliverToBlueprint
            && selected_count < WHEELBARROW_PREFERRED_MIN_BATCH_SIZE;
        if is_small_batch && eval.pending_for < SINGLE_BATCH_WAIT_SECS {
            continue;
        }

        let mut items = Vec::with_capacity(selected_count);
        let mut source_sum = Vec2::ZERO;
        for candidate in nearby_items.iter().take(selected_count) {
            items.push(candidate.entity);
            source_sum += candidate.pos;
        }
        let source_pos = source_sum / selected_count as f32;

        let min_wb_distance = available_wheelbarrows
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
                hard_min: eval.hard_min,
                pending_for: eval.pending_for,
                is_small_batch,
            },
            score,
        ));
    }

    candidates.sort_by(|(_, s1), (_, s2)| s2.partial_cmp(s1).unwrap_or(Ordering::Equal));
    (
        candidates,
        eligible_requests,
        bucket_items_total,
        candidates_after_top_k,
        pending_secs_total,
    )
}
