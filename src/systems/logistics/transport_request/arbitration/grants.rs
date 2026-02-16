//! 手押し車 lease の割り当て

use std::cmp::Ordering;

use crate::constants::*;
use crate::systems::logistics::transport_request::WheelbarrowLease;
use bevy::prelude::*;

use super::types::BatchCandidate;

pub fn grant_leases(
    candidates: &[(BatchCandidate, f32)],
    available_wheelbarrows: &mut Vec<(Entity, Vec2)>,
    now: f64,
    commands: &mut Commands,
    q_stockpiles: &Query<(
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    _cache: &crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache,
    q_incoming: &Query<&crate::relationships::IncomingDeliveries>,
) -> u32 {
    let mut leases_granted = 0u32;
    let mut chosen_cells = std::collections::HashMap::<Entity, usize>::new();

    for (candidate, _score) in candidates {
        if available_wheelbarrows.is_empty() {
            break;
        }

        let mut final_destination = candidate.destination.clone();

        // Stockpile への搬入の場合、実積載数に基づいて最適なセルを再選択する
        if let crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(_) =
            &candidate.destination
        {
            let count = candidate.items.len();
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
                final_destination =
                    crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(
                        cell,
                    );
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
        let (wb_entity, _wb_pos) = available_wheelbarrows.remove(idx);

        commands
            .entity(candidate.request_entity)
            .insert(WheelbarrowLease {
                wheelbarrow: wb_entity,
                items: candidate.items.clone(),
                source_pos: candidate.source_pos,
                destination: final_destination,
                lease_until: now + WHEELBARROW_LEASE_DURATION_SECS,
            });

        leases_granted += 1;
        debug!(
            "WB Arbitration: lease granted to request {:?} -> wb {:?} (small_batch={})",
            candidate.request_entity, wb_entity, candidate.is_small_batch,
        );
    }

    leases_granted
}
