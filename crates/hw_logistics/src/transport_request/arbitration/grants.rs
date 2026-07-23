//! 手押し車 lease の割り当て

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::relationships::{IncomingDeliveries, StoredItems};

use crate::stockpile_policy::stockpile_owner_accepts_item;
use crate::transport_request::{WheelbarrowDestination, WheelbarrowLease};
use crate::types::{BelongsTo, ResourceItem, ResourceType};
use crate::zone::{Stockpile, StockpilePolicy};

use super::WheelbarrowArbitrationOutcome;
use super::candidates::evaluate_stockpile_cell;
use super::types::BatchCandidate;

#[derive(Default)]
pub(super) struct GrantStats {
    pub leases_granted: u32,
    pub items_deduped: u32,
    pub candidates_dropped_by_dedup: u32,
    pub lease_duration_total_secs: f64,
}

pub(super) struct GrantLeaseQueries<'a> {
    pub q_stockpiles: &'a Query<
        'a,
        'a,
        (
            &'static Stockpile,
            Option<&'static StockpilePolicy>,
            Option<&'static StoredItems>,
        ),
    >,
    pub q_incoming: &'a Query<'a, 'a, &'static IncomingDeliveries>,
    pub q_resource_items: &'a Query<'a, 'a, &'static ResourceItem>,
    pub q_belongs: &'a Query<'a, 'a, &'static BelongsTo>,
    pub q_transforms: &'a Query<'a, 'a, &'static Transform>,
}

#[derive(Debug, Clone, Copy)]
struct CellGrantOption {
    entity: Entity,
    available: usize,
    pos: Vec2,
}

fn cell_order(left: CellGrantOption, right: CellGrantOption) -> Ordering {
    left.pos
        .x
        .total_cmp(&right.pos.x)
        .then_with(|| left.pos.y.total_cmp(&right.pos.y))
        .then_with(|| left.entity.index_u32().cmp(&right.entity.index_u32()))
        .then_with(|| {
            left.entity
                .generation()
                .to_bits()
                .cmp(&right.entity.generation().to_bits())
        })
}

fn choose_stockpile_cell(
    options: impl Iterator<Item = CellGrantOption>,
    requested: usize,
) -> Option<(CellGrantOption, usize)> {
    let mut best_fit = None::<CellGrantOption>;
    let mut best_partial = None::<CellGrantOption>;
    for option in options.filter(|option| option.available > 0) {
        if option.available >= requested {
            let replace = best_fit.is_none_or(|best| {
                option.available < best.available
                    || (option.available == best.available && cell_order(option, best).is_lt())
            });
            if replace {
                best_fit = Some(option);
            }
        } else {
            let replace = best_partial.is_none_or(|best| {
                option.available > best.available
                    || (option.available == best.available && cell_order(option, best).is_lt())
            });
            if replace {
                best_partial = Some(option);
            }
        }
    }
    best_fit
        .or(best_partial)
        .map(|option| (option, requested.min(option.available)))
}

pub fn grant_leases(
    candidates: &[(BatchCandidate, f32)],
    available_wheelbarrows: &mut Vec<(Entity, Vec2)>,
    now: f64,
    commands: &mut Commands,
    queries: GrantLeaseQueries<'_>,
    outcomes: &mut HashMap<Entity, WheelbarrowArbitrationOutcome>,
) -> GrantStats {
    let mut stats = GrantStats::default();
    let mut consumed_items = HashSet::<Entity>::new();
    let mut chosen_cells = HashMap::<Entity, HashMap<ResourceType, usize>>::new();

    for (candidate, _score) in candidates {
        if available_wheelbarrows.is_empty() {
            outcomes.insert(
                candidate.request_entity,
                WheelbarrowArbitrationOutcome::ArbitrationContention,
            );
            continue;
        }

        let mut lease_items: Vec<Entity> = candidate
            .items
            .iter()
            .copied()
            .filter(|item| !consumed_items.contains(item))
            .collect();
        let removed_by_dedup = candidate.items.len().saturating_sub(lease_items.len());
        stats.items_deduped = stats.items_deduped.saturating_add(removed_by_dedup as u32);
        if lease_items.len() < candidate.hard_min {
            stats.candidates_dropped_by_dedup = stats.candidates_dropped_by_dedup.saturating_add(1);
            outcomes.insert(
                candidate.request_entity,
                WheelbarrowArbitrationOutcome::ArbitrationContention,
            );
            continue;
        }

        let mut final_destination = candidate.destination;
        let destination_pos;

        if let WheelbarrowDestination::Stockpile(_) = &candidate.destination {
            let direct_cell = match candidate.destination {
                WheelbarrowDestination::Stockpile(cell) => cell,
                _ => unreachable!("matched stockpile destination"),
            };
            let cells = if candidate.group_cells.is_empty() {
                std::slice::from_ref(&direct_cell)
            } else {
                candidate.group_cells.as_slice()
            };
            let mut options = Vec::with_capacity(cells.len());
            let mut blocked_by_reservation = false;
            let mut saw_stockpile = false;
            let mut saw_missing_transform = false;
            for &cell in cells {
                let cell_owner = queries.q_belongs.get(cell).ok().map(|belongs| belongs.0);
                let owners_compatible = lease_items.iter().all(|item| {
                    let item_owner = queries.q_belongs.get(*item).ok().map(|belongs| belongs.0);
                    stockpile_owner_accepts_item(item_owner, cell_owner)
                });
                if !owners_compatible {
                    continue;
                }
                let Some(evaluation) = evaluate_stockpile_cell(
                    cell,
                    candidate.resource_type,
                    candidate.receiver_priority,
                    queries.q_stockpiles,
                    queries.q_incoming,
                    queries.q_resource_items,
                    chosen_cells.get(&cell),
                ) else {
                    continue;
                };
                saw_stockpile = true;
                blocked_by_reservation |= evaluation.blocked_by_reservation;
                let Ok(transform) = queries.q_transforms.get(cell) else {
                    saw_missing_transform |= evaluation.available > 0;
                    continue;
                };
                options.push(CellGrantOption {
                    entity: cell,
                    available: evaluation.available,
                    pos: transform.translation.truncate(),
                });
            }

            let Some((chosen, allowed)) =
                choose_stockpile_cell(options.into_iter(), lease_items.len())
            else {
                outcomes.insert(
                    candidate.request_entity,
                    if !saw_stockpile || saw_missing_transform {
                        WheelbarrowArbitrationOutcome::StaleInput
                    } else if blocked_by_reservation {
                        WheelbarrowArbitrationOutcome::CapacityReserved
                    } else {
                        WheelbarrowArbitrationOutcome::NoDestinationCapacity
                    },
                );
                continue;
            };
            lease_items.truncate(allowed);
            if lease_items.len() < candidate.hard_min {
                outcomes.insert(
                    candidate.request_entity,
                    if blocked_by_reservation {
                        WheelbarrowArbitrationOutcome::CapacityReserved
                    } else {
                        WheelbarrowArbitrationOutcome::NoDestinationCapacity
                    },
                );
                continue;
            }
            final_destination = WheelbarrowDestination::Stockpile(chosen.entity);
            destination_pos = chosen.pos;
            *chosen_cells
                .entry(chosen.entity)
                .or_default()
                .entry(candidate.resource_type)
                .or_insert(0) += lease_items.len();
        } else {
            let Some(pos) = destination_world_pos(final_destination, queries.q_transforms) else {
                outcomes.insert(
                    candidate.request_entity,
                    WheelbarrowArbitrationOutcome::StaleInput,
                );
                continue;
            };
            destination_pos = pos;
        }

        let best_idx = available_wheelbarrows
            .iter()
            .enumerate()
            .min_by(|(_, (entity_a, pos_a)), (_, (entity_b, pos_b))| {
                let da = pos_a.distance_squared(candidate.source_pos);
                let db = pos_b.distance_squared(candidate.source_pos);
                da.total_cmp(&db)
                    .then_with(|| entity_a.index_u32().cmp(&entity_b.index_u32()))
                    .then_with(|| {
                        entity_a
                            .generation()
                            .to_bits()
                            .cmp(&entity_b.generation().to_bits())
                    })
            })
            .map(|(idx, _)| idx);

        let Some(idx) = best_idx else {
            break;
        };
        let (wb_entity, wb_pos) = available_wheelbarrows.remove(idx);
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
        outcomes.insert(
            candidate.request_entity,
            WheelbarrowArbitrationOutcome::LeaseGranted,
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    fn option(index: u32, available: usize, x: f32) -> CellGrantOption {
        CellGrantOption {
            entity: entity(index),
            available,
            pos: Vec2::new(x, 0.0),
        }
    }

    #[test]
    fn picks_smallest_cell_that_fits_the_whole_batch() {
        let (chosen, allowed) =
            choose_stockpile_cell([option(1, 5, 0.0), option(2, 3, 1.0)].into_iter(), 3).unwrap();
        assert_eq!(chosen.entity, entity(2));
        assert_eq!(allowed, 3);
    }

    #[test]
    fn truncates_to_the_largest_partial_cell() {
        let (chosen, allowed) =
            choose_stockpile_cell([option(1, 1, 0.0), option(2, 2, 1.0)].into_iter(), 3).unwrap();
        assert_eq!(chosen.entity, entity(2));
        assert_eq!(allowed, 2);
    }

    #[test]
    fn stable_position_breaks_equal_capacity_ties() {
        let (chosen, _) =
            choose_stockpile_cell([option(1, 2, 1.0), option(2, 2, -1.0)].into_iter(), 2).unwrap();
        assert_eq!(chosen.entity, entity(2));
    }
}
