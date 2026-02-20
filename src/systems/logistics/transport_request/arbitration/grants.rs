//! 手押し車 lease の割り当て

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::constants::*;
use crate::systems::logistics::transport_request::{WheelbarrowDestination, WheelbarrowLease};
use bevy::prelude::*;

use super::types::BatchCandidate;

#[derive(Default)]
pub struct GrantStats {
    pub leases_granted: u32,
    pub items_deduped: u32,
    pub candidates_dropped_by_dedup: u32,
    pub lease_duration_total_secs: f64,
}

pub fn grant_leases(
    candidates: &[(BatchCandidate, f32)],
    available_wheelbarrows: &mut Vec<(Entity, Vec2)>,
    now: f64,
    commands: &mut Commands,
    q_stockpiles: &Query<(
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_incoming: &Query<&crate::relationships::IncomingDeliveries>,
    q_transforms: &Query<&Transform>,
) -> GrantStats {
    let mut stats = GrantStats::default();
    let mut consumed_items = HashSet::<Entity>::new();
    let mut chosen_cells = std::collections::HashMap::<Entity, usize>::new();

    for (candidate, _score) in candidates {
        if available_wheelbarrows.is_empty() {
            break;
        }

        let lease_items: Vec<Entity> = candidate
            .items
            .iter()
            .copied()
            .filter(|item| !consumed_items.contains(item))
            .collect();
        let removed_by_dedup = candidate.items.len().saturating_sub(lease_items.len());
        stats.items_deduped = stats.items_deduped.saturating_add(removed_by_dedup as u32);
        if lease_items.len() < candidate.hard_min {
            stats.candidates_dropped_by_dedup = stats.candidates_dropped_by_dedup.saturating_add(1);
            continue;
        }

        let mut final_destination = candidate.destination.clone();

        // Stockpile への搬入の場合、実積載数に基づいて最適なセルを再選択する
        if let WheelbarrowDestination::Stockpile(_) = &candidate.destination {
            let count = lease_items.len();
            let mut greedy_cell = None; // 残容量が count 以上かつ最小のセル
            let mut fallback_cell = None; // それ以外の空きありセルの中で残容量最小

            for &cell in &candidate.group_cells {
                if let Ok((stock, stored_opt)) = q_stockpiles.get(cell) {
                    let current = stored_opt.map(|s| s.len()).unwrap_or(0);

                    // インフライト（過去フレーム予約）
                    let incoming = q_incoming
                        .get(cell)
                        .ok()
                        .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                        .unwrap_or(0);
                    let anticipated = current + incoming;
                    // このフレームでのアセット済み数（多重割り当て防止）
                    let already_chosen = *chosen_cells.get(&cell).unwrap_or(&0);

                    let free = stock.capacity.saturating_sub(anticipated + already_chosen);
                    if free == 0 {
                        continue;
                    }

                    if count <= free {
                        // 1往復で埋まる（または隙間なく入る）セル -> 残容量最小を優先
                        match greedy_cell {
                            Some((_, best_free)) if free >= best_free => {}
                            _ => greedy_cell = Some((cell, free)),
                        }
                    } else {
                        // 1往復では埋まらない -> 改めて残容量最小を優先
                        match fallback_cell {
                            Some((_, best_free)) if free >= best_free => {}
                            _ => fallback_cell = Some((cell, free)),
                        }
                    }
                }
            }

            if let Some((cell, _)) = greedy_cell.or(fallback_cell) {
                final_destination = WheelbarrowDestination::Stockpile(cell);
                *chosen_cells.entry(cell).or_insert(0) += count;
            }
        }

        let best_idx = available_wheelbarrows
            .iter()
            .enumerate()
            .min_by(|(_, (_, pos_a)), (_, (_, pos_b))| {
                let da = pos_a.distance_squared(candidate.source_pos);
                let db = pos_b.distance_squared(candidate.source_pos);
                da.partial_cmp(&db).unwrap_or(Ordering::Equal)
            })
            .map(|(idx, _)| idx);

        let Some(idx) = best_idx else {
            break;
        };
        let (wb_entity, wb_pos) = available_wheelbarrows.remove(idx);
        let destination_pos =
            destination_world_pos(final_destination, q_transforms).unwrap_or(candidate.source_pos);
        let lease_duration =
            compute_lease_duration_secs(wb_pos, candidate.source_pos, destination_pos);
        consumed_items.extend(lease_items.iter().copied());

        commands
            .entity(candidate.request_entity)
            .insert(WheelbarrowLease {
                wheelbarrow: wb_entity,
                items: lease_items,
                source_pos: candidate.source_pos,
                destination: final_destination,
                lease_until: now + lease_duration,
            });

        stats.leases_granted = stats.leases_granted.saturating_add(1);
        stats.lease_duration_total_secs += lease_duration;
        debug!(
            "WB Arbitration: lease granted to request {:?} -> wb {:?} (small_batch={}, pending_for={:.1}, duration={:.1}s)",
            candidate.request_entity,
            wb_entity,
            candidate.is_small_batch,
            candidate.pending_for,
            lease_duration,
        );
    }

    stats
}

fn destination_world_pos(
    destination: WheelbarrowDestination,
    q_transforms: &Query<&Transform>,
) -> Option<Vec2> {
    let destination_entity = match destination {
        WheelbarrowDestination::Stockpile(entity) | WheelbarrowDestination::Blueprint(entity) => {
            entity
        }
        WheelbarrowDestination::Mixer { entity, .. } => entity,
    };
    q_transforms
        .get(destination_entity)
        .ok()
        .map(|transform| transform.translation.truncate())
}

fn compute_lease_duration_secs(wb_pos: Vec2, source_pos: Vec2, destination_pos: Vec2) -> f64 {
    let travel_speed = (SOUL_SPEED_BASE * SOUL_SPEED_WHEELBARROW_MULTIPLIER).max(1.0);
    let total_distance = wb_pos.distance(source_pos) + source_pos.distance(destination_pos);
    let travel_time = (total_distance / travel_speed) as f64;
    (travel_time
        + travel_time * WHEELBARROW_LEASE_BUFFER_RATIO
        + WHEELBARROW_LEASE_MIN_DURATION_SECS)
        .clamp(
            WHEELBARROW_LEASE_MIN_DURATION_SECS,
            WHEELBARROW_LEASE_MAX_DURATION_SECS,
        )
}
