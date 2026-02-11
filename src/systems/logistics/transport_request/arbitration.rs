//! Wheelbarrow Arbitration System
//!
//! producer が request を出し終えた後に実行され、「どの DepositToStockpile request に
//! 手押し車を割り当てるか」を一括で決定する。
//! スコアベースの Greedy 割り当てにより、全体最適に近い手押し車配分を行う。

use bevy::prelude::*;
use std::collections::HashSet;

use crate::constants::*;
use crate::relationships::{ParkedAt, PushedBy, StoredItems};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportRequest, TransportRequestKind, TransportRequestState,
    WheelbarrowLease,
};
use crate::systems::logistics::{BelongsTo, ReservedForTask, ResourceItem, Stockpile, Wheelbarrow};

use super::metrics::TransportRequestMetrics;

/// バッチ候補の評価結果
struct BatchCandidate {
    request_entity: Entity,
    items: Vec<Entity>,
    source_pos: Vec2,
    dest_stockpile: Entity,
}

/// 手押し車仲裁システム
///
/// Arbitrate フェーズで実行。DepositToStockpile の Pending request に対して
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
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let now = time.elapsed_secs_f64();

    // --- Phase 1: 期限切れ lease を除去 ---
    let mut used_wheelbarrows = HashSet::new();
    for (req_entity, _req, _state, _demand, _transform, lease_opt) in q_requests.iter() {
        if let Some(lease) = lease_opt {
            if lease.lease_until < now {
                commands.entity(req_entity).remove::<WheelbarrowLease>();
            } else {
                // まだ有効な lease の wheelbarrow は使用中
                used_wheelbarrows.insert(lease.wheelbarrow);
            }
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

    for (req_entity, req, state, _demand, _transform, lease_opt) in q_requests.iter() {
        // eligible: DepositToStockpile, Pending, lease なし, loadable
        if req.kind != TransportRequestKind::DepositToStockpile {
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

        let stockpile_entity = req.anchor;
        let item_owner = Some(req.issued_by);
        let resource_type = req.resource_type;

        // dest_capacity を確認
        let dest_capacity = if let Ok((stock, stored_opt)) = q_stockpiles.get(stockpile_entity) {
            let current = stored_opt.map(|s| s.len()).unwrap_or(0);
            if current >= stock.capacity {
                0
            } else {
                stock.capacity - current
            }
        } else {
            continue;
        };

        let max_items = dest_capacity.min(WHEELBARROW_CAPACITY);
        if max_items < WHEELBARROW_MIN_BATCH_SIZE {
            continue;
        }

        // free_items から resource_type + owner が一致する地面アイテムを収集
        let stockpile_pos = _transform.translation.truncate();
        let mut item_candidates: Vec<(Entity, Vec2, f32)> = q_free_items
            .iter()
            .filter(|(_, _, vis, res)| {
                **vis != Visibility::Hidden && res.0 == resource_type
            })
            // 地面アイテムのみ（StoredIn なし）
            .filter(|(e, _, _, _)| q_stored_in.get(*e).is_err())
            // owner 一致
            .filter(|(e, _, _, _)| {
                let belongs = q_belongs.get(*e).ok().map(|b| b.0);
                item_owner == belongs
            })
            .map(|(e, t, _, _)| {
                let pos = t.translation.truncate();
                let dist_sq = pos.distance_squared(stockpile_pos);
                (e, pos, dist_sq)
            })
            .filter(|(_, _, d)| *d <= search_radius_sq)
            .collect();

        item_candidates
            .sort_by(|(_, _, d1), (_, _, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal));

        let items: Vec<Entity> = item_candidates
            .iter()
            .take(max_items)
            .map(|(e, _, _)| *e)
            .collect();

        if items.len() < WHEELBARROW_MIN_BATCH_SIZE {
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
        let score = score_candidate(items.len() as f32, priority as f32, min_wb_distance);

        candidates.push((
            BatchCandidate {
                request_entity: req_entity,
                items,
                source_pos,
                dest_stockpile: stockpile_entity,
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
            dest_stockpile: candidate.dest_stockpile,
            lease_until: now + WHEELBARROW_LEASE_DURATION_SECS,
        });

        leases_granted += 1;
        debug!(
            "WB Arbitration: lease granted to request {:?} -> wb {:?} (batch={})",
            candidate.request_entity,
            wb_entity,
            leases_granted,
        );
    }

    // --- Phase 5: メトリクス更新 ---
    let active_leases = used_wheelbarrows.len() as u32 + leases_granted;
    metrics.wheelbarrow_leases_active = active_leases;
    metrics.wheelbarrow_leases_granted_this_frame = leases_granted;
}

/// スコア計算
fn score_candidate(batch_size: f32, priority: f32, wb_distance: f32) -> f32 {
    batch_size * WHEELBARROW_SCORE_BATCH_SIZE
        + priority * WHEELBARROW_SCORE_PRIORITY
        - wb_distance * WHEELBARROW_SCORE_DISTANCE
}
