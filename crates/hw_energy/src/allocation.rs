use bevy::prelude::Entity;

use crate::{
    POWER_ALLOCATION_EPSILON, POWER_RESTORE_MARGIN, PowerAllocationMode, PowerPriority,
    PowerShedReason, PowerSupplyState,
};

#[derive(Debug, Clone, Copy)]
pub struct PowerConsumerAllocationInput {
    pub entity: Entity,
    pub grid_pos: (i32, i32),
    pub demand: f32,
    pub priority: PowerPriority,
    pub previous_state: Option<PowerSupplyState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerConsumerAllocation {
    pub entity: Entity,
    pub state: PowerSupplyState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerAllocationResult {
    pub consumers: Vec<PowerConsumerAllocation>,
    pub total_demand: f32,
    pub served_demand: f32,
    pub all_supplied: bool,
}

pub fn allocate_power(
    mode: PowerAllocationMode,
    generation: f32,
    consumers: &[PowerConsumerAllocationInput],
) -> PowerAllocationResult {
    let generation = usable_generation(generation);
    let mut ordered = consumers.to_vec();
    match mode {
        PowerAllocationMode::LegacyAllOrNone => ordered.sort_by_key(spatial_key),
        PowerAllocationMode::PriorityPrefix => ordered.sort_by_key(priority_key),
    }

    let total_demand = ordered
        .iter()
        .filter_map(|consumer| valid_demand(consumer.demand))
        .sum();

    match mode {
        PowerAllocationMode::LegacyAllOrNone => allocate_legacy(generation, ordered, total_demand),
        PowerAllocationMode::PriorityPrefix => {
            allocate_priority_prefix(generation, ordered, total_demand)
        }
    }
}

fn allocate_legacy(
    generation: f32,
    ordered: Vec<PowerConsumerAllocationInput>,
    total_demand: f32,
) -> PowerAllocationResult {
    let has_invalid = ordered
        .iter()
        .any(|consumer| valid_demand(consumer.demand).is_none());
    let supply_all = capacity_fits(generation, total_demand);
    let consumers = ordered
        .into_iter()
        .map(|consumer| PowerConsumerAllocation {
            entity: consumer.entity,
            state: if valid_demand(consumer.demand).is_none() {
                PowerSupplyState::InvalidDemand
            } else if supply_all {
                PowerSupplyState::Supplied
            } else {
                PowerSupplyState::Shed {
                    reason: PowerShedReason::LegacyGlobalDeficit,
                }
            },
        })
        .collect();

    PowerAllocationResult {
        consumers,
        total_demand,
        served_demand: if supply_all { total_demand } else { 0.0 },
        all_supplied: supply_all && !has_invalid,
    }
}

fn allocate_priority_prefix(
    generation: f32,
    ordered: Vec<PowerConsumerAllocationInput>,
    total_demand: f32,
) -> PowerAllocationResult {
    let mut allocations = Vec::with_capacity(ordered.len());
    let mut cumulative_demand = 0.0;
    let mut served_demand = 0.0;
    let mut blocked_reason = None;
    let mut has_invalid = false;

    for consumer in ordered {
        let Some(demand) = valid_demand(consumer.demand) else {
            has_invalid = true;
            allocations.push(PowerConsumerAllocation {
                entity: consumer.entity,
                state: PowerSupplyState::InvalidDemand,
            });
            continue;
        };

        cumulative_demand += demand;
        let state = if let Some(reason) = blocked_reason {
            PowerSupplyState::Shed { reason }
        } else if !capacity_fits(generation, cumulative_demand) {
            blocked_reason = Some(PowerShedReason::InsufficientGeneration);
            PowerSupplyState::Shed {
                reason: PowerShedReason::InsufficientGeneration,
            }
        } else if demand > POWER_ALLOCATION_EPSILON
            && matches!(
                consumer.previous_state,
                Some(PowerSupplyState::Shed {
                    reason: PowerShedReason::InsufficientGeneration
                        | PowerShedReason::RestoreMargin,
                })
            )
            && !capacity_fits(generation, cumulative_demand + POWER_RESTORE_MARGIN)
        {
            blocked_reason = Some(PowerShedReason::RestoreMargin);
            PowerSupplyState::Shed {
                reason: PowerShedReason::RestoreMargin,
            }
        } else {
            served_demand += demand;
            PowerSupplyState::Supplied
        };
        allocations.push(PowerConsumerAllocation {
            entity: consumer.entity,
            state,
        });
    }

    PowerAllocationResult {
        all_supplied: blocked_reason.is_none() && !has_invalid,
        consumers: allocations,
        total_demand,
        served_demand,
    }
}

fn valid_demand(demand: f32) -> Option<f32> {
    (demand.is_finite() && demand >= 0.0).then_some(demand)
}

fn usable_generation(generation: f32) -> f32 {
    if generation.is_finite() && generation > 0.0 {
        generation
    } else {
        0.0
    }
}

fn capacity_fits(generation: f32, demand: f32) -> bool {
    generation + POWER_ALLOCATION_EPSILON >= demand
}

fn spatial_key(consumer: &PowerConsumerAllocationInput) -> (i32, i32, u64) {
    (
        consumer.grid_pos.1,
        consumer.grid_pos.0,
        consumer.entity.to_bits(),
    )
}

fn priority_key(consumer: &PowerConsumerAllocationInput) -> (u8, i32, i32, u64) {
    (
        consumer.priority.allocation_rank(),
        consumer.grid_pos.1,
        consumer.grid_pos.0,
        consumer.entity.to_bits(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity")
    }

    fn input(
        index: u32,
        priority: PowerPriority,
        grid_pos: (i32, i32),
        demand: f32,
    ) -> PowerConsumerAllocationInput {
        PowerConsumerAllocationInput {
            entity: entity(index),
            grid_pos,
            demand,
            priority,
            previous_state: None,
        }
    }

    fn states(result: &PowerAllocationResult) -> Vec<(Entity, PowerSupplyState)> {
        result
            .consumers
            .iter()
            .map(|allocation| (allocation.entity, allocation.state))
            .collect()
    }

    #[test]
    fn priority_and_spatial_order_are_deterministic() {
        let consumers = [
            input(1, PowerPriority::Low, (0, 0), 1.0),
            input(2, PowerPriority::High, (8, 4), 1.0),
            input(3, PowerPriority::Normal, (2, 1), 1.0),
            input(4, PowerPriority::High, (3, 2), 1.0),
        ];
        let shuffled = [consumers[2], consumers[0], consumers[3], consumers[1]];

        let first = allocate_power(PowerAllocationMode::PriorityPrefix, 2.0, &consumers);
        let second = allocate_power(PowerAllocationMode::PriorityPrefix, 2.0, &shuffled);

        assert_eq!(states(&first), states(&second));
        assert_eq!(
            states(&first),
            vec![
                (entity(4), PowerSupplyState::Supplied),
                (entity(2), PowerSupplyState::Supplied),
                (
                    entity(3),
                    PowerSupplyState::Shed {
                        reason: PowerShedReason::InsufficientGeneration,
                    },
                ),
                (
                    entity(1),
                    PowerSupplyState::Shed {
                        reason: PowerShedReason::InsufficientGeneration,
                    },
                ),
            ]
        );
    }

    #[test]
    fn strict_prefix_never_skips_a_large_consumer_for_a_smaller_tail() {
        let result = allocate_power(
            PowerAllocationMode::PriorityPrefix,
            0.9,
            &[
                input(1, PowerPriority::High, (0, 0), 0.8),
                input(2, PowerPriority::Normal, (0, 1), 1.0),
                input(3, PowerPriority::Low, (0, 2), 0.1),
            ],
        );

        assert_eq!(result.served_demand, 0.8);
        assert_eq!(states(&result)[0].1, PowerSupplyState::Supplied);
        assert!(matches!(
            states(&result)[1].1,
            PowerSupplyState::Shed { .. }
        ));
        assert!(matches!(
            states(&result)[2].1,
            PowerSupplyState::Shed { .. }
        ));
    }

    #[test]
    fn cold_start_uses_exact_capacity_but_known_shed_waits_for_margin() {
        let mut consumer = input(1, PowerPriority::Normal, (0, 0), 1.0);
        let cold = allocate_power(PowerAllocationMode::PriorityPrefix, 1.0, &[consumer]);
        assert_eq!(states(&cold)[0].1, PowerSupplyState::Supplied);

        consumer.previous_state = Some(PowerSupplyState::Supplied);
        let shed = allocate_power(PowerAllocationMode::PriorityPrefix, 0.9, &[consumer]);
        assert_eq!(
            states(&shed)[0].1,
            PowerSupplyState::Shed {
                reason: PowerShedReason::InsufficientGeneration,
            }
        );

        consumer.previous_state = Some(states(&shed)[0].1);
        let waiting = allocate_power(
            PowerAllocationMode::PriorityPrefix,
            1.0 + POWER_RESTORE_MARGIN / 2.0,
            &[consumer],
        );
        assert_eq!(
            states(&waiting)[0].1,
            PowerSupplyState::Shed {
                reason: PowerShedReason::RestoreMargin,
            }
        );
        let restored = allocate_power(
            PowerAllocationMode::PriorityPrefix,
            1.0 + POWER_RESTORE_MARGIN,
            &[consumer],
        );
        assert_eq!(states(&restored)[0].1, PowerSupplyState::Supplied);
    }

    #[test]
    fn legacy_mode_is_all_or_none_and_ignores_priority() {
        let consumers = [
            input(1, PowerPriority::Low, (0, 0), 0.6),
            input(2, PowerPriority::High, (1, 0), 0.4),
        ];
        let exact = allocate_power(PowerAllocationMode::LegacyAllOrNone, 1.0, &consumers);
        assert!(exact.all_supplied);
        assert!(
            exact
                .consumers
                .iter()
                .all(|allocation| allocation.state == PowerSupplyState::Supplied)
        );

        let deficit = allocate_power(PowerAllocationMode::LegacyAllOrNone, 0.99, &consumers);
        assert_eq!(deficit.served_demand, 0.0);
        assert!(deficit.consumers.iter().all(|allocation| {
            allocation.state
                == PowerSupplyState::Shed {
                    reason: PowerShedReason::LegacyGlobalDeficit,
                }
        }));
    }

    #[test]
    fn switching_from_legacy_to_priority_is_a_cold_start() {
        let mut consumer = input(1, PowerPriority::Normal, (0, 0), 1.0);
        consumer.previous_state = Some(PowerSupplyState::Shed {
            reason: PowerShedReason::LegacyGlobalDeficit,
        });

        let result = allocate_power(PowerAllocationMode::PriorityPrefix, 1.0, &[consumer]);

        assert_eq!(states(&result)[0].1, PowerSupplyState::Supplied);
    }

    #[test]
    fn invalid_demand_is_isolated_from_totals_and_valid_consumers() {
        for demand in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let result = allocate_power(
                PowerAllocationMode::PriorityPrefix,
                1.0,
                &[
                    input(1, PowerPriority::High, (0, 0), demand),
                    input(2, PowerPriority::Normal, (1, 0), 1.0),
                ],
            );
            assert_eq!(result.total_demand, 1.0);
            assert_eq!(result.served_demand, 1.0);
            assert!(!result.all_supplied);
            assert_eq!(states(&result)[0].1, PowerSupplyState::InvalidDemand);
            assert_eq!(states(&result)[1].1, PowerSupplyState::Supplied);
        }
    }

    #[test]
    fn empty_grid_is_fully_supplied() {
        let result = allocate_power(PowerAllocationMode::PriorityPrefix, 0.0, &[]);
        assert!(result.all_supplied);
        assert_eq!(result.total_demand, 0.0);
        assert_eq!(result.served_demand, 0.0);
        assert!(result.consumers.is_empty());
    }
}
