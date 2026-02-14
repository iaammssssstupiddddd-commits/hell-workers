//! Wheelbarrow Arbitration System
//!
//! producer が request を出し終えた後に実行され、「どの request に
//! 手押し車を割り当てるか」を一括で決定する。
//! スコアベースの Greedy 割り当てにより、全体最適に近い手押し車配分を行う。

use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::time::Instant;

use crate::constants::*;
use crate::relationships::{ParkedAt, PushedBy, StoredIn, StoredItems};
use crate::systems::jobs::Blueprint;
use crate::systems::logistics::transport_request::{
    can_complete_pick_drop_to_blueprint, can_complete_pick_drop_to_point,
    TransportDemand, TransportRequest, TransportRequestKind, TransportRequestState,
    WheelbarrowDestination, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::systems::logistics::{
    BelongsTo, ReservedForTask, ResourceItem, ResourceType, Stockpile, Wheelbarrow,
};

use super::metrics::TransportRequestMetrics;

/// バッチ候補の評価結果
struct BatchCandidate {
    request_entity: Entity,
    items: Vec<Entity>,
    source_pos: Vec2,
    destination: WheelbarrowDestination,
    is_small_batch: bool,
}

#[derive(Clone, Copy)]
struct FreeItemSnapshot {
    entity: Entity,
    pos: Vec2,
    resource_type: ResourceType,
    owner: Option<Entity>,
    is_ground: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ItemBucketKey {
    Resource(ResourceType),
    ResourceOwnerGround {
        resource_type: ResourceType,
        owner: Option<Entity>,
    },
}

struct RequestEvalContext {
    request_entity: Entity,
    request_pos: Vec2,
    resource_type: ResourceType,
    destination: WheelbarrowDestination,
    max_items: usize,
    hard_min: usize,
    pending_for: f64,
    priority: u32,
    bucket_key: ItemBucketKey,
}

#[derive(Clone, Copy)]
struct NearbyItem {
    entity: Entity,
    pos: Vec2,
    dist_sq: f32,
}

#[derive(Clone, Copy, Debug)]
struct HeapEntry {
    snapshot_idx: usize,
    dist_sq: f32,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot_idx == other.snapshot_idx
            && self.dist_sq.total_cmp(&other.dist_sq) == Ordering::Equal
    }
}

impl Eq for HeapEntry {}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.dist_sq
            .total_cmp(&other.dist_sq)
            .then_with(|| self.snapshot_idx.cmp(&other.snapshot_idx))
    }
}

/// 手押し車仲裁システム
///
/// Arbitrate フェーズで実行。対象 request の Pending タスクに対して
/// 手押し車を一括割り当てする。
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
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
        ),
    >,
    q_belongs: Query<&BelongsTo>,
    q_stored_in: Query<&StoredIn>,
    q_stockpiles: Query<(&Stockpile, Option<&StoredItems>)>,
    q_blueprints: Query<&Blueprint>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let arbitration_started_at = Instant::now();
    let now = time.elapsed_secs_f64();

    // --- Phase 1: 期限切れ lease と pending_since を更新 ---
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
                // まだ有効な lease の wheelbarrow は使用中
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
            commands
                .entity(req_entity)
                .remove::<WheelbarrowPendingSince>();
        }
    }

    // --- Phase 2: 利用可能な wheelbarrow を収集 ---
    let mut available_wheelbarrows: Vec<(Entity, Vec2)> = q_wheelbarrows
        .iter()
        .filter(|(e, _)| !used_wheelbarrows.contains(e))
        .map(|(e, t)| (e, t.translation.truncate()))
        .collect();

    let mut candidates: Vec<(BatchCandidate, f32)> = Vec::new();
    let mut leases_granted = 0u32;
    let mut eligible_requests = 0u32;
    let mut bucket_items_total = 0u32;
    let mut candidates_after_top_k = 0u32;

    if !available_wheelbarrows.is_empty() {
        // --- Phase 3: free item を1回走査してバケット化 ---
        let (free_items, by_resource, by_resource_owner_ground) =
            build_free_item_buckets(&q_free_items, &q_belongs, &q_stored_in);
        let search_radius_sq = (TILE_SIZE * 10.0) * (TILE_SIZE * 10.0);

        // --- Phase 4: eligible requests を抽出してバッチ候補を評価 ---
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
                &q_belongs,
                &q_stockpiles,
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

            let nearby_items = collect_top_k_nearest(
                bucket,
                &free_items,
                eval.request_pos,
                search_radius_sq,
                WHEELBARROW_ARBITRATION_TOP_K,
            );
            candidates_after_top_k += nearby_items.len() as u32;
            if nearby_items.is_empty() {
                continue;
            }

            if eval.resource_type.requires_wheelbarrow() {
                // その場ピック→ドロップで完了できるケースは猫車仲裁対象から外す
                let pick_drop_possible = match eval.destination {
                    WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Mixer { .. } => {
                        nearby_items.iter().any(|candidate| {
                            can_complete_pick_drop_to_point(candidate.pos, eval.request_pos)
                        })
                    }
                    WheelbarrowDestination::Blueprint(blueprint_entity) => {
                        q_blueprints.get(blueprint_entity).ok().is_some_and(|bp| {
                            nearby_items.iter().any(|candidate| {
                                can_complete_pick_drop_to_blueprint(
                                    candidate.pos,
                                    &bp.occupied_grids,
                                )
                            })
                        })
                    }
                };

                if pick_drop_possible {
                    continue;
                }
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
            let mut source_sum = Vec2::ZERO;
            for candidate in nearby_items.iter().take(selected_count) {
                items.push(candidate.entity);
                source_sum += candidate.pos;
            }
            let source_pos = source_sum / selected_count as f32;

            // 最近の wheelbarrow までの距離
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

        // --- Phase 5: スコア降順にソート → Greedy 割り当て ---
        candidates.sort_by(|(_, s1), (_, s2)| s2.partial_cmp(s1).unwrap_or(Ordering::Equal));
    }

    for (candidate, _score) in candidates {
        if available_wheelbarrows.is_empty() {
            break;
        }

        // 最近の wheelbarrow を選択
        let best_idx = available_wheelbarrows
            .iter()
            .enumerate()
            .min_by(|(_, (_, pos_a)), (_, (_, pos_b))| {
                let da = pos_a.distance_squared(candidate.source_pos);
                let db = pos_b.distance_squared(candidate.source_pos);
                da.partial_cmp(&db).unwrap_or(Ordering::Equal)
            })
            .map(|(idx, _)| idx);

        let Some(idx) = best_idx else { break };
        let (wb_entity, _wb_pos) = available_wheelbarrows.remove(idx);

        // WheelbarrowLease を insert
        commands
            .entity(candidate.request_entity)
            .insert(WheelbarrowLease {
                wheelbarrow: wb_entity,
                items: candidate.items,
                source_pos: candidate.source_pos,
                destination: candidate.destination,
                lease_until: now + WHEELBARROW_LEASE_DURATION_SECS,
            });

        leases_granted += 1;
        debug!(
            "WB Arbitration: lease granted to request {:?} -> wb {:?} (small_batch={})",
            candidate.request_entity, wb_entity, candidate.is_small_batch,
        );
    }

    // --- Phase 6: メトリクス更新 ---
    let active_leases = used_wheelbarrows.len() as u32 + leases_granted;
    metrics.wheelbarrow_leases_active = active_leases;
    metrics.wheelbarrow_leases_granted_this_frame = leases_granted;
    metrics.wheelbarrow_arb_eligible_requests = eligible_requests;
    metrics.wheelbarrow_arb_bucket_items_total = bucket_items_total;
    metrics.wheelbarrow_arb_candidates_after_topk = candidates_after_top_k;
    metrics.wheelbarrow_arb_elapsed_ms = arbitration_started_at.elapsed().as_secs_f32() * 1000.0;
}

fn build_free_item_buckets(
    q_free_items: &Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
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

fn build_request_eval_context(
    req_entity: Entity,
    req: &TransportRequest,
    state: &TransportRequestState,
    demand: &TransportDemand,
    transform: &Transform,
    lease_opt: Option<&WheelbarrowLease>,
    pending_since_opt: Option<&WheelbarrowPendingSince>,
    now: f64,
    q_belongs: &Query<&BelongsTo>,
    q_stockpiles: &Query<(&Stockpile, Option<&StoredItems>)>,
) -> Option<RequestEvalContext> {
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

            // dest_capacity を確認（グループ対応・型互換のみ）
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

fn collect_top_k_nearest(
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

/// スコア計算
fn score_candidate(batch_size: f32, priority: f32, wb_distance: f32, is_small_batch: bool) -> f32 {
    let mut score = batch_size * WHEELBARROW_SCORE_BATCH_SIZE
        + priority * WHEELBARROW_SCORE_PRIORITY
        - wb_distance * WHEELBARROW_SCORE_DISTANCE;

    if is_small_batch {
        score -= WHEELBARROW_SCORE_SMALL_BATCH_PENALTY;
    }

    score
}

