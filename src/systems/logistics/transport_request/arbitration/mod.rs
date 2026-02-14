//! Wheelbarrow Arbitration System
//!
//! producer が request を出し終えた後に実行され、「どの request に
//! 手押し車を割り当てるか」を一括で決定する。
//! スコアベースの Greedy 割り当てにより、全体最適に近い手押し車配分を行う。

mod candidates;
mod grants;
mod types;

use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Instant;

use crate::constants::*;
use crate::relationships::{ParkedAt, PushedBy};
use crate::systems::jobs::Blueprint;
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportRequest, TransportRequestKind, TransportRequestState,
    WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::systems::logistics::{ReservedForTask, Wheelbarrow};
use bevy::prelude::*;

use super::metrics::TransportRequestMetrics;
use candidates::{
    build_free_item_buckets, build_request_eval_context, collect_top_k_nearest, is_pick_drop_possible,
    score_candidate,
};
use grants::grant_leases;
use types::{BatchCandidate, ItemBucketKey};

pub fn wheelbarrow_arbitration_system(
    mut commands: Commands,
    time: Res<Time>,
    q_requests: Query<(
        Entity,
        &TransportRequest,
        &TransportRequestState,
        &TransportDemand,
        &Transform,
        Option<&WheelbarrowLease>,
        Option<&WheelbarrowPendingSince>,
    )>,
    q_wheelbarrows: Query<
        (Entity, &Transform),
        (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
    >,
    q_free_items: Query<
        (Entity, &Transform, &Visibility, &crate::systems::logistics::ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
        ),
    >,
    q_belongs: Query<&crate::systems::logistics::BelongsTo>,
    q_stored_in: Query<&crate::relationships::StoredIn>,
    q_stockpiles: Query<(
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_blueprints: Query<&Blueprint>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let arbitration_started_at = Instant::now();
    let now = time.elapsed_secs_f64();

    let used_wheelbarrows =
        update_lease_state(&mut commands, &q_requests, &q_free_items, &q_wheelbarrows, now);

    let mut available_wheelbarrows: Vec<(Entity, Vec2)> = q_wheelbarrows
        .iter()
        .filter(|(e, _)| !used_wheelbarrows.contains(e))
        .map(|(e, t)| (e, t.translation.truncate()))
        .collect();

    let (candidates, eligible_requests, bucket_items_total, candidates_after_top_k) =
        collect_candidates(
            &q_requests,
            &q_free_items,
            &q_belongs,
            &q_stored_in,
            &q_stockpiles,
            &q_blueprints,
            &available_wheelbarrows,
            now,
        );

    let leases_granted = grant_leases(
        &candidates,
        &mut available_wheelbarrows,
        now,
        &mut commands,
    );

    update_metrics(
        &mut metrics,
        used_wheelbarrows.len() as u32 + leases_granted,
        leases_granted,
        eligible_requests,
        bucket_items_total,
        candidates_after_top_k,
        arbitration_started_at,
    );
}

fn update_lease_state(
    commands: &mut Commands,
    q_requests: &Query<(
        Entity,
        &crate::systems::logistics::transport_request::TransportRequest,
        &TransportRequestState,
        &TransportDemand,
        &Transform,
        Option<&WheelbarrowLease>,
        Option<&WheelbarrowPendingSince>,
    )>,
    q_free_items: &Query<
        (Entity, &Transform, &Visibility, &crate::systems::logistics::ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
        ),
    >,
    q_wheelbarrows: &Query<
        (Entity, &Transform),
        (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
    >,
    now: f64,
) -> HashSet<Entity> {
    let mut used_wheelbarrows = HashSet::new();

    for (req_entity, req, state, _demand, _transform, lease_opt, pending_since_opt) in
        q_requests.iter()
    {
        if let Some(lease) = lease_opt {
            let min_valid_items = if req.resource_type.requires_wheelbarrow() {
                1
            } else {
                WHEELBARROW_MIN_BATCH_SIZE
            };
            let valid_item_count = lease
                .items
                .iter()
                .filter(|item| {
                    q_free_items
                        .get(**item)
                        .ok()
                        .is_some_and(|(_, _, vis, _)| *vis != Visibility::Hidden)
                })
                .count();
            let lease_stale = q_wheelbarrows.get(lease.wheelbarrow).is_err()
                || valid_item_count < min_valid_items;

            if lease.lease_until < now || lease_stale {
                commands.entity(req_entity).remove::<WheelbarrowLease>();
            } else {
                used_wheelbarrows.insert(lease.wheelbarrow);
            }
        }

        if *state == TransportRequestState::Pending {
            if pending_since_opt.is_none() {
                commands
                    .entity(req_entity)
                    .insert(WheelbarrowPendingSince(now));
            }
        } else if pending_since_opt.is_some() {
            commands.entity(req_entity).remove::<WheelbarrowPendingSince>();
        }
    }

    used_wheelbarrows
}

fn collect_candidates(
    q_requests: &Query<(
        Entity,
        &TransportRequest,
        &TransportRequestState,
        &TransportDemand,
        &Transform,
        Option<&WheelbarrowLease>,
        Option<&WheelbarrowPendingSince>,
    )>,
    q_free_items: &Query<
        (Entity, &Transform, &Visibility, &crate::systems::logistics::ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
        ),
    >,
    q_belongs: &Query<&crate::systems::logistics::BelongsTo>,
    q_stored_in: &Query<&crate::relationships::StoredIn>,
    q_stockpiles: &Query<(
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_blueprints: &Query<&Blueprint>,
    available_wheelbarrows: &[(Entity, Vec2)],
    now: f64,
) -> (Vec<(BatchCandidate, f32)>, u32, u32, u32) {
    let mut candidates: Vec<(BatchCandidate, f32)> = Vec::new();
    let mut eligible_requests = 0u32;
    let mut bucket_items_total = 0u32;
    let mut candidates_after_top_k = 0u32;

    if available_wheelbarrows.is_empty() {
        return (candidates, eligible_requests, bucket_items_total, candidates_after_top_k);
    }

    let (free_items, by_resource, by_resource_owner_ground) =
        build_free_item_buckets(q_free_items, q_belongs, q_stored_in);
    let search_radius_sq = (crate::constants::TILE_SIZE * 10.0)
        * (crate::constants::TILE_SIZE * 10.0);

    for (req_entity, req, state, demand, transform, lease_opt, pending_since_opt) in
        q_requests.iter()
    {
        let Some(eval) = build_request_eval_context(
            req_entity,
            req,
            state,
            demand,
            transform,
            lease_opt,
            pending_since_opt,
            now,
            q_belongs,
            q_stockpiles,
        ) else {
            continue;
        };
        eligible_requests += 1;

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

        // 近傍に候補がいない場合のみ、猫車必須の request では探索範囲を全域へ拡張する。
        // これにより近距離 pick&drop 判定は維持しつつ、遠距離搬送でも猫車 lease を作れる。
        if nearby_items.is_empty()
            && req.resource_type.requires_wheelbarrow()
            && matches!(
                req.kind,
                TransportRequestKind::DeliverToBlueprint | TransportRequestKind::DeliverToMixerSolid
            )
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

        if is_pick_drop_possible(&eval, &nearby_items, q_blueprints) {
            continue;
        }

        let selected_count = nearby_items.len().min(eval.max_items);
        if selected_count < eval.hard_min {
            continue;
        }

        let is_small_batch = eval.resource_type.requires_wheelbarrow()
            && selected_count < WHEELBARROW_PREFERRED_MIN_BATCH_SIZE;
        if is_small_batch && eval.pending_for < SINGLE_BATCH_WAIT_SECS {
            continue;
        }

        let mut items = Vec::with_capacity(selected_count);
        let mut source_sum = bevy::prelude::Vec2::ZERO;
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
            is_small_batch,
        );

        candidates.push((
            BatchCandidate {
                request_entity: eval.request_entity,
                items,
                source_pos,
                destination: eval.destination,
                is_small_batch,
            },
            score,
        ));
    }

    candidates.sort_by(|(_, s1), (_, s2)| s2.partial_cmp(s1).unwrap_or(Ordering::Equal));
    (candidates, eligible_requests, bucket_items_total, candidates_after_top_k)
}

fn update_metrics(
    metrics: &mut TransportRequestMetrics,
    active_leases: u32,
    leases_granted: u32,
    eligible_requests: u32,
    bucket_items_total: u32,
    candidates_after_top_k: u32,
    arbitration_started_at: std::time::Instant,
) {
    metrics.wheelbarrow_leases_active = active_leases;
    metrics.wheelbarrow_leases_granted_this_frame = leases_granted;
    metrics.wheelbarrow_arb_eligible_requests = eligible_requests;
    metrics.wheelbarrow_arb_bucket_items_total = bucket_items_total;
    metrics.wheelbarrow_arb_candidates_after_topk = candidates_after_top_k;
    metrics.wheelbarrow_arb_elapsed_ms =
        arbitration_started_at.elapsed().as_secs_f32() * 1000.0;
}
