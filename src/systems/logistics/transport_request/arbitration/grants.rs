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
) -> u32 {
    let mut leases_granted = 0u32;

    for (candidate, _score) in candidates {
        if available_wheelbarrows.is_empty() {
            break;
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
                destination: candidate.destination.clone(),
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
