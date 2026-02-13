//! Wheelbarrow Arbitration System
//!
//! producer が request を出し終えた後に実行され、「どの request に
//! 手押し車を割り当てるか」を一括で決定する。
//! スコアベースの Greedy 割り当てにより、全体最適に近い手押し車配分を行う。

use bevy::prelude::*;
use std::collections::HashSet;

use crate::constants::*;
use crate::relationships::{ParkedAt, PushedBy, StoredItems};
use crate::systems::jobs::Blueprint;
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportRequest, TransportRequestKind, TransportRequestState,
    WheelbarrowDestination, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::systems::logistics::{BelongsTo, ReservedForTask, ResourceItem, Stockpile, Wheelbarrow};
use crate::world::map::WorldMap;

use super::metrics::TransportRequestMetrics;

/// バッチ候補の評価結果
struct BatchCandidate {
    request_entity: Entity,
    items: Vec<Entity>,
    source_pos: Vec2,
    destination: WheelbarrowDestination,
    is_small_batch: bool,
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
    q_stored_in: Query<&crate::relationships::StoredIn>,
    q_stockpiles: Query<(&Stockpile, Option<&StoredItems>)>,
    q_blueprints: Query<&Blueprint>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
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
            commands.entity(req_entity).remove::<WheelbarrowPendingSince>();
        }
    }

    // --- Phase 2: 利用可能な wheelbarrow を収集 ---
    let mut available_wheelbarrows: Vec<(Entity, Vec2)> = q_wheelbarrows
        .iter()
        .filter(|(e, _)| !used_wheelbarrows.contains(e))
        .map(|(e, t)| (e, t.translation.truncate()))
        .collect();

    if available_wheelbarrows.is_empty() {
        return;
    }

    // --- Phase 3: eligible requests を抽出してバッチ候補を評価 ---
    let search_radius_sq = (TILE_SIZE * 10.0) * (TILE_SIZE * 10.0);

    let mut candidates: Vec<(BatchCandidate, f32)> = Vec::new();

    for (req_entity, req, state, demand, transform, lease_opt, pending_since_opt) in
        q_requests.iter()
    {
        // eligible: request kind, Pending, lease なし, loadable
        let eligible_kind = match req.kind {
            TransportRequestKind::DepositToStockpile => true,
            TransportRequestKind::DeliverToBlueprint | TransportRequestKind::DeliverToMixerSolid => {
                req.resource_type.requires_wheelbarrow()
            }
            _ => false,
        };
        if !eligible_kind {
            continue;
        }
        if *state != TransportRequestState::Pending {
            continue;
        }
        if lease_opt.is_some() {
            continue;
        }
        if !req.resource_type.is_loadable() {
            continue;
        }

        let mut owner_filter_enabled = false;
        let mut owner = None;
        let mut ground_only = false;

        let (destination, max_items) = match req.kind {
            TransportRequestKind::DepositToStockpile => {
                owner_filter_enabled = true;
                owner = q_belongs.get(req.anchor).ok().map(|b| b.0);
                ground_only = true;

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
                            let current = stored_opt.map(|s| s.len()).unwrap_or(0);
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
                    let type_ok = stock.resource_type.is_none()
                        || stock.resource_type == Some(req.resource_type);
                    if !type_ok {
                        continue;
                    }
                    let current = stored_opt.map(|s| s.len()).unwrap_or(0);
                    let free = stock.capacity.saturating_sub(current);
                    (req.anchor, free)
                } else {
                    continue;
                };

                let max_items = dest_capacity.min(WHEELBARROW_CAPACITY);
                (
                    WheelbarrowDestination::Stockpile(dest_stockpile),
                    max_items,
                )
            }
            TransportRequestKind::DeliverToBlueprint => {
                let max_items = (demand.remaining() as usize).min(WHEELBARROW_CAPACITY);
                (WheelbarrowDestination::Blueprint(req.anchor), max_items)
            }
            TransportRequestKind::DeliverToMixerSolid => {
                let max_items = (demand.remaining() as usize).min(WHEELBARROW_CAPACITY);
                (
                    WheelbarrowDestination::Mixer {
                        entity: req.anchor,
                        resource_type: req.resource_type,
                    },
                    max_items,
                )
            }
            _ => continue,
        };

        let hard_min = if req.resource_type.requires_wheelbarrow() {
            1
        } else {
            WHEELBARROW_MIN_BATCH_SIZE
        };
        if max_items < hard_min {
            continue;
        }

        // free_items から resource_type が一致するアイテムを収集
        let request_pos = transform.translation.truncate();
        let mut item_candidates: Vec<(Entity, Vec2, f32)> = q_free_items
            .iter()
            .filter(|(_, _, vis, res)| **vis != Visibility::Hidden && res.0 == req.resource_type)
            .filter(|(e, _, _, _)| !ground_only || q_stored_in.get(*e).is_err())
            .filter(|(e, _, _, _)| {
                if !owner_filter_enabled {
                    return true;
                }
                let belongs = q_belongs.get(*e).ok().map(|b| b.0);
                owner == belongs
            })
            .map(|(e, t, _, _)| {
                let pos = t.translation.truncate();
                let dist_sq = pos.distance_squared(request_pos);
                (e, pos, dist_sq)
            })
            .filter(|(_, _, d)| *d <= search_radius_sq)
            .collect();

        item_candidates.sort_by(|(_, _, d1), (_, _, d2)| {
            d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
        });

        if req.resource_type.requires_wheelbarrow() {
            // その場ピック→ドロップで完了できるケースは猫車仲裁対象から外す
            let pick_drop_possible = match destination {
                WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Mixer { .. } => {
                    item_candidates.iter().any(|(_, pos, _)| {
                        can_complete_pick_drop_to_point(*pos, request_pos)
                    })
                }
                WheelbarrowDestination::Blueprint(blueprint_entity) => {
                    q_blueprints.get(blueprint_entity).ok().is_some_and(|bp| {
                        item_candidates.iter().any(|(_, pos, _)| {
                            can_complete_pick_drop_to_blueprint(*pos, &bp.occupied_grids)
                        })
                    })
                }
            };

            if pick_drop_possible {
                continue;
            }
        }

        let items: Vec<Entity> = item_candidates
            .iter()
            .take(max_items)
            .map(|(e, _, _)| *e)
            .collect();

        if items.len() < hard_min {
            continue;
        }

        let pending_for = pending_since_opt.map(|p| now - p.0).unwrap_or(0.0);
        let is_small_batch = req.resource_type.requires_wheelbarrow()
            && items.len() < WHEELBARROW_PREFERRED_MIN_BATCH_SIZE;
        if is_small_batch && pending_for < SINGLE_BATCH_WAIT_SECS {
            continue;
        }

        // source_pos = アイテム重心
        let source_pos = {
            let sum: Vec2 = item_candidates
                .iter()
                .take(items.len())
                .map(|(_, pos, _)| *pos)
                .fold(Vec2::ZERO, |acc, p| acc + p);
            sum / items.len() as f32
        };

        // 最近の wheelbarrow までの距離
        let min_wb_distance = available_wheelbarrows
            .iter()
            .map(|(_, wb_pos)| wb_pos.distance(source_pos))
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(f32::MAX);

        let priority = req.priority as u32;
        let score = score_candidate(
            items.len() as f32,
            priority as f32,
            min_wb_distance,
            is_small_batch,
        );

        candidates.push((
            BatchCandidate {
                request_entity: req_entity,
                items,
                source_pos,
                destination,
                is_small_batch,
            },
            score,
        ));
    }

    // --- Phase 4: スコア降順にソート → Greedy 割り当て ---
    candidates.sort_by(|(_, s1), (_, s2)| s2.partial_cmp(s1).unwrap_or(std::cmp::Ordering::Equal));

    let mut leases_granted = 0u32;

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
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx);

        let Some(idx) = best_idx else { break };
        let (wb_entity, _wb_pos) = available_wheelbarrows.remove(idx);

        // WheelbarrowLease を insert
        commands.entity(candidate.request_entity).insert(WheelbarrowLease {
            wheelbarrow: wb_entity,
            items: candidate.items,
            source_pos: candidate.source_pos,
            destination: candidate.destination,
            lease_until: now + WHEELBARROW_LEASE_DURATION_SECS,
        });

        leases_granted += 1;
        debug!(
            "WB Arbitration: lease granted to request {:?} -> wb {:?} (small_batch={})",
            candidate.request_entity,
            wb_entity,
            candidate.is_small_batch,
        );
    }

    // --- Phase 5: メトリクス更新 ---
    let active_leases = used_wheelbarrows.len() as u32 + leases_granted;
    metrics.wheelbarrow_leases_active = active_leases;
    metrics.wheelbarrow_leases_granted_this_frame = leases_granted;
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

fn can_complete_pick_drop_to_point(source_pos: Vec2, destination_pos: Vec2) -> bool {
    let source_grid = WorldMap::world_to_grid(source_pos);
    // 実タスク条件に合わせる:
    // 1) source に隣接して拾える立ち位置が存在し
    // 2) その立ち位置が destination へのドロップ判定を満たす
    for dx in -1..=1 {
        for dy in -1..=1 {
            let stand_pos = WorldMap::grid_to_world(source_grid.0 + dx, source_grid.1 + dy);
            if stand_pos.distance(destination_pos) < TILE_SIZE * 1.8 {
                return true;
            }
        }
    }
    false
}

fn can_complete_pick_drop_to_blueprint(source_pos: Vec2, occupied_grids: &[(i32, i32)]) -> bool {
    let source_grid = WorldMap::world_to_grid(source_pos);
    for dx in -1..=1 {
        for dy in -1..=1 {
            let stand_grid = (source_grid.0 + dx, source_grid.1 + dy);
            if occupied_grids.contains(&stand_grid) {
                continue;
            }
            let stand_pos = WorldMap::grid_to_world(stand_grid.0, stand_grid.1);
            if occupied_grids.iter().any(|&(gx, gy)| {
                let bp_pos = WorldMap::grid_to_world(gx, gy);
                stand_pos.distance(bp_pos) < TILE_SIZE * 1.5
            }) {
                return true;
            }
        }
    }
    false
}
